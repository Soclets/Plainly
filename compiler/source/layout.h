// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// D54's message layout, shared by the compiler's messages and the runtime's `check failed:` messages, so both are
// laid out by the same code. `Out` is `std::string` in the compiler and the runtime's own buffer, which has the same
// `append` and `push_back`, since the runtime doesn't use the C++ library's strings.

#pragma once

#include <cstdint>
#include <string_view>

namespace unio::layout {

// The digits of a number, most significant first; `sign` is -1 when it's negative, so the most negative `Whole`
// stays exact.
template <typename Out> void append_digits(Out& out, std::int64_t number, int sign) {
    char digits[20]; // NOLINT(cppcoreguidelines-avoid-c-arrays): no library strings in the runtime
    int count = 0;
    while (number != 0) {
        digits[count++] = static_cast<char>('0' + sign * static_cast<int>(number % 10)); // NOLINT
        number /= 10;
    }
    while (count > 0) {
        out.push_back(digits[--count]); // NOLINT
    }
}

// trace: D54
template <typename Out> void append_number(Out& out, std::int64_t number) {
    if (number < 0) {
        out.push_back('-');
        // Counting down from the negative keeps -9223372036854775808 exact.
        append_digits(out, number, -1);
        return;
    }
    if (number == 0) {
        out.push_back('0');
        return;
    }
    append_digits(out, number, 1);
}

// The first line: kind, then the place twice, for tools and for people (D54).
template <typename Out>
void append_heading(Out& out, std::string_view kind, std::string_view file, std::uint32_t line, std::uint32_t column) {
    out.append(kind);
    out.append(": ");
    out.append(file);
    out.push_back(':');
    append_number(out, line);
    out.push_back(':');
    append_number(out, column);
    out.append(", line ");
    append_number(out, line);
    out.append(" column ");
    append_number(out, column);
    out.push_back('\n');
}

// A source line, then `^` under `length` characters from `column`. The line number takes at least two places.
template <typename Out>
void append_source(Out& out, std::uint32_t line, std::string_view text, std::uint32_t column, std::uint32_t length) {
    std::size_t digits = 1;
    for (std::uint32_t rest = line; rest >= 10; rest /= 10) {
        ++digits;
    }
    const std::size_t width = digits < 2 ? 2 : digits;
    out.append(width - digits, ' ');
    append_number(out, line);
    out.append(" | ");
    out.append(text);
    out.push_back('\n');
    out.append(width, ' ');
    out.append(" | ");
    out.append(column - 1, ' ');
    out.append(length == 0 ? 1 : length, '^');
    out.push_back('\n');
}

// A labelled line, its text starting one space after the longest label, `because:` (D86).
template <typename Out> void append_label(Out& out, std::string_view label, std::string_view text) {
    constexpr std::size_t text_column = 10;
    out.push_back(' ');
    out.append(label);
    out.push_back(':');
    out.append(text_column - 2 - label.size(), ' ');
    out.append(text);
    out.push_back('\n');
}

} // namespace unio::layout
