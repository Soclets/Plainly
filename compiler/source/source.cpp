// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "source.h"

#include <algorithm>
#include <cerrno>
#include <cstring>
#include <fstream>
#include <sstream>

namespace unio {

Place span(Place first, Place last) {
    const std::uint32_t end = std::max(first.end(), last.end());
    return Place{.file = first.file, .offset = first.offset, .length = end - first.offset};
}

namespace {

bool is_utf8(std::string_view text) {
    std::size_t index = 0;
    while (index < text.size()) {
        const auto byte = static_cast<unsigned char>(text[index]);
        std::size_t extra = 0;
        if (byte < 0x80) {
            extra = 0;
        } else if ((byte & 0xE0U) == 0xC0U && byte >= 0xC2U) {
            extra = 1;
        } else if ((byte & 0xF0U) == 0xE0U) {
            extra = 2;
        } else if ((byte & 0xF8U) == 0xF0U && byte <= 0xF4U) {
            extra = 3;
        } else {
            return false;
        }
        for (std::size_t next = 1; next <= extra; ++next) {
            if (index + next >= text.size() || (static_cast<unsigned char>(text[index + next]) & 0xC0U) != 0x80U) {
                return false;
            }
        }
        index += extra + 1;
    }
    return true;
}

} // namespace

std::expected<std::uint32_t, std::string> Sources::read(const std::string& path, const std::string& name) {
    // trace: D35, D136
    if (!path.ends_with(".unio")) {
        return std::unexpected(std::string("a Unio file's name ends in `.unio`"));
    }
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        const int error = errno;
        return std::unexpected(std::string(std::strerror(error)));
    }
    std::ostringstream contents;
    contents << file.rdbuf();
    if (file.bad()) {
        return std::unexpected(std::string("it couldn't be read"));
    }
    std::string text = contents.str();
    if (!is_utf8(text)) {
        return std::unexpected(std::string("it isn't UTF-8 text"));
    }
    // trace: D144
    std::string normalized;
    normalized.reserve(text.size());
    for (std::size_t index = 0; index < text.size(); ++index) {
        if (text[index] == '\r' && index + 1 < text.size() && text[index + 1] == '\n') {
            continue;
        }
        normalized.push_back(text[index]);
    }
    return add(name, std::move(normalized));
}

std::uint32_t Sources::add(std::string name, std::string text) {
    SourceFile file{.name = std::move(name), .text = std::move(text), .line_starts = {0}};
    for (std::size_t index = 0; index < file.text.size(); ++index) {
        if (file.text[index] == '\n') {
            file.line_starts.push_back(static_cast<std::uint32_t>(index + 1));
        }
    }
    files_.push_back(std::move(file));
    return static_cast<std::uint32_t>(files_.size() - 1);
}

LineColumn Sources::line_column(Place place) const {
    const SourceFile& source = files_.at(place.file);
    const auto after = std::ranges::upper_bound(source.line_starts, place.offset);
    const auto line = static_cast<std::uint32_t>(after - source.line_starts.begin());
    const std::uint32_t start = source.line_starts.at(line - 1);
    return LineColumn{.line = line, .column = place.offset - start + 1};
}

std::uint32_t Sources::line_count(std::uint32_t file) const {
    const SourceFile& source = files_.at(file);
    auto count = static_cast<std::uint32_t>(source.line_starts.size());
    if (!source.text.empty() && source.text.back() == '\n') {
        --count;
    }
    return count;
}

std::string_view Sources::line_text(std::uint32_t file, std::uint32_t line) const {
    const SourceFile& source = files_.at(file);
    if (line == 0 || line > source.line_starts.size()) {
        return {};
    }
    const std::uint32_t start = source.line_starts.at(line - 1);
    std::size_t end = source.text.find('\n', start);
    if (end == std::string::npos) {
        end = source.text.size();
    }
    return std::string_view(source.text).substr(start, end - start);
}

std::string_view Sources::text(Place place) const {
    return std::string_view(files_.at(place.file).text).substr(place.offset, place.length);
}

} // namespace unio
