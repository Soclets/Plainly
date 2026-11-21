// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// The program's files, as read, and places in them.

#pragma once

#include <cstdint>
#include <expected>
#include <string>
#include <string_view>
#include <vector>

namespace unio {

// A place in a file: where a message's `^` goes. Offsets are bytes into the file's text after D144's `\r\n` became
// `\n`.
struct Place {
    std::uint32_t file = 0;
    std::uint32_t offset = 0;
    std::uint32_t length = 0;

    [[nodiscard]] std::uint32_t end() const { return offset + length; }
};

// Joins two places in the same file into one running from the start of the first to the end of the second.
Place span(Place first, Place last);

struct SourceFile {
    std::string name; // as the command named it, or beside it in its folder (D64)
    std::string text;
    std::vector<std::uint32_t> line_starts;
};

struct LineColumn {
    std::uint32_t line = 1;
    std::uint32_t column = 1;
};

class Sources {
public:
    // Reads a file, or gives the `what:` of D103's `can't read:` message.
    std::expected<std::uint32_t, std::string> read(const std::string& path, const std::string& name);
    std::uint32_t add(std::string name, std::string text);

    [[nodiscard]] const SourceFile& file(std::uint32_t index) const { return files_.at(index); }
    [[nodiscard]] std::size_t size() const { return files_.size(); }
    [[nodiscard]] LineColumn line_column(Place place) const;
    // The text of line `line` (from 1), without its line break.
    [[nodiscard]] std::string_view line_text(std::uint32_t file, std::uint32_t line) const;
    [[nodiscard]] std::uint32_t line_count(std::uint32_t file) const;
    [[nodiscard]] std::string_view text(Place place) const;

private:
    std::vector<SourceFile> files_;
};

} // namespace unio
