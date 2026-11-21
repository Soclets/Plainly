// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// The runtime every Unio program is linked with: showing and asking (D6, D20), texts and their owners (D130), the
// memory cap (D134), `be.Whole` (D95), `random` (D26), and the messages of checks that fail at runtime (D46, D54).
// It uses only the C library, so a program starts fast and needs nothing else installed.

#include "../source/layout.h"

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <string_view>

#include <pthread.h>
#include <sys/sysctl.h>
#include <unistd.h>

namespace {

// A text: how many owners it has, its length, then its bytes. A negative owner count marks a text that's never
// freed, as the constants in a program are (D130).
struct Text {
    std::int64_t owners;
    std::int64_t length;
};

// Where a check is, for its message: laid out as the compiler's codegen writes it.
struct Site {
    const char* file;
    const char* source_line;
    const char* detail; // a `?? crash`'s `what:`, or the procedure a call is inside
    std::uint32_t line;
    std::uint32_t column;
    std::uint32_t length;
    std::uint32_t kind;
};

enum class SiteKind : std::uint8_t { add, subtract, multiply, negate, memory, random_order, crash, depth };

const char* bytes(const Text* text) {
    // The bytes follow the header directly; the header is two 8-byte fields, so they're aligned for chars.
    return reinterpret_cast<const char*>(text + 1); // NOLINT(cppcoreguidelines-pro-type-reinterpret-cast)
}

char* bytes(Text* text) {
    return reinterpret_cast<char*>(text + 1); // NOLINT(cppcoreguidelines-pro-type-reinterpret-cast)
}

// A growing buffer with the few operations layout.h needs, kept in static storage.
class Buffer {
public:
    void push_back(char character) {
        if (size_ < capacity) {
            data_.at(size_++) = character;
        }
    }
    void append(std::string_view text) {
        for (const char character : text) {
            push_back(character);
        }
    }
    void append(std::size_t count, char character) {
        for (std::size_t index = 0; index < count; ++index) {
            push_back(character);
        }
    }
    [[nodiscard]] const char* data() const { return data_.data(); }
    [[nodiscard]] std::size_t size() const { return size_; }

private:
    static constexpr std::size_t capacity = 8192;
    std::array<char, capacity> data_{};
    std::size_t size_ = 0;
};

void write_all(int descriptor, const char* data, std::size_t size) {
    while (size > 0) {
        const ssize_t written = write(descriptor, data, size);
        if (written <= 0) {
            return;
        }
        data += written; // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
        size -= static_cast<std::size_t>(written);
    }
}

// Standard output, buffered, and whether its last line is still open (D104).
class Output {
public:
    void write(const char* data, std::size_t size) {
        if (size == 0) {
            return;
        }
        if (used_ + size > capacity) {
            flush();
        }
        if (size >= capacity) {
            write_all(1, data, size);
        } else {
            std::memcpy(buffer_.data() + used_, data, size); // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
            used_ += size;
        }
        line_open_ = data[size - 1] != '\n'; // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
    }
    void flush() {
        write_all(1, buffer_.data(), used_);
        used_ = 0;
    }
    // trace: D104
    void end_open_line() {
        if (line_open_) {
            write("\n", 1);
        }
        flush();
    }
    void close_line() { line_open_ = false; }

private:
    static constexpr std::size_t capacity = 65536;
    std::array<char, capacity> buffer_{};
    std::size_t used_ = 0;
    bool line_open_ = false;
};

// The program has one standard output and one standard input, so the runtime keeps one of each.
Output output; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables)

// Standard input, read a line at a time (D20).
class Input {
public:
    // Reads up to the next line break, calling `take` for each piece; false when no more input was there at all.
    template <typename Take> bool read_line(Take take) {
        bool any = false;
        while (true) {
            if (start_ == end_) {
                const ssize_t count = read(0, buffer_.data(), capacity);
                if (count <= 0) {
                    return any;
                }
                start_ = 0;
                end_ = static_cast<std::size_t>(count);
            }
            any = true;
            const char* begin = buffer_.data() + start_; // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
            const auto* line_break = static_cast<const char*>(std::memchr(begin, '\n', end_ - start_));
            if (line_break != nullptr) {
                take(begin, static_cast<std::size_t>(line_break - begin));
                start_ += static_cast<std::size_t>(line_break - begin) + 1;
                return true;
            }
            take(begin, end_ - start_);
            start_ = end_;
        }
    }

private:
    static constexpr std::size_t capacity = 65536;
    std::array<char, capacity> buffer_{};
    std::size_t start_ = 0;
    std::size_t end_ = 0;
};

Input input; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables)

void write_message(const Buffer& message) {
    output.end_open_line();
    write_all(2, message.data(), message.size());
}

void heading(Buffer& message, std::string_view kind, const Site* site) {
    unio::layout::append_heading(message, kind, site->file, site->line, site->column);
    unio::layout::append_source(message, site->line, site->source_line, site->column, site->length);
}

// --- Memory (D115, D122, D134) ------------------------------------------------------------------------------------

// How much the program holds, and the most it may (D134), for the whole program.
std::uint64_t memory_in_use = 0; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables)
std::uint64_t memory_cap = 0;    // NOLINT(cppcoreguidelines-avoid-non-const-global-variables)

void append_amount(Buffer& message, std::uint64_t amount) {
    if (amount < 1024) {
        unio::layout::append_number(message, static_cast<std::int64_t>(amount));
        message.append(amount == 1 ? " byte" : " bytes");
        return;
    }
    static constexpr std::array<std::string_view, 4> units{"KiB", "MiB", "GiB", "TiB"};
    std::size_t unit = 0;
    std::uint64_t scale = 1024;
    while (unit < 3 && amount >= scale * 1024) {
        scale *= 1024;
        ++unit;
    }
    // One decimal place, rounded.
    const std::uint64_t tenths = (amount * 10 + scale / 2) / scale;
    unio::layout::append_number(message, static_cast<std::int64_t>(tenths / 10));
    message.push_back('.');
    message.push_back(static_cast<char>('0' + tenths % 10));
    message.push_back(' ');
    message.append(units.at(unit));
}

[[noreturn]] void out_of_memory(const Site* site, std::uint64_t needed) {
    static Buffer message;
    heading(message, "check failed", site);
    unio::layout::append_label(message, "what", "the program ran out of memory");
    Buffer when;
    when.append("needed ");
    append_amount(when, needed);
    when.append(" more, with ");
    append_amount(when, memory_in_use);
    when.append(" of the ");
    append_amount(when, memory_cap);
    when.append(" it may use on this computer in use");
    unio::layout::append_label(message, "when", std::string_view(when.data(), when.size()));
    unio::layout::append_label(message, "why",
                               "everything the program keeps is still in use, so none of it can be freed.");
    write_message(message);
    std::_Exit(4);
}

Text* allocate_text(std::int64_t length, const Site* site) {
    const auto size = sizeof(Text) + static_cast<std::uint64_t>(length);
    if (memory_in_use + size > memory_cap) {
        out_of_memory(site, size);
    }
    // The runtime is where Unio's own ownership (D130) meets the machine: a text is freed when its last owner ends.
    auto* text = // NOLINT(cppcoreguidelines-owning-memory): owned by the text's Unio owners, not by C++
        static_cast<Text*>(std::malloc(size)); // NOLINT(cppcoreguidelines-no-malloc,cppcoreguidelines-owning-memory)
    if (text == nullptr) {
        out_of_memory(site, size);
    }
    memory_in_use += size;
    text->owners = 1;
    text->length = length;
    return text;
}

void free_text(Text* text) {
    memory_in_use -= sizeof(Text) + static_cast<std::uint64_t>(text->length);
    std::free(text); // NOLINT(cppcoreguidelines-no-malloc,cppcoreguidelines-owning-memory)
}

} // namespace

// libc++'s hardened checks (Building rule 3) end here when one fails. libc++ lets a program supply this function,
// so the runtime needs no C++ library: a failed check in the runtime is a bug in it, reported as one (D82, D103).
// NOLINTNEXTLINE(bugprone-std-namespace-modification): libc++ documents supplying this as the way to customize it
namespace std::inline __1 {
[[noreturn]] void __libcpp_verbose_abort(const char* format, ...) noexcept { // NOLINT: libc++'s own signature
    output.end_open_line();
    constexpr std::string_view text =
        "internal error: Unlo\\n's runtime itself went wrong, not your program\n"
        " what:    a check inside the runtime failed\n"
        " why:     this is a bug in Unlo\\n. Please report it at https://github.com/Soclets/Unio-lang/issues, with the "
        "program that caused it.\n";
    (void)format;
    write_all(2, text.data(), text.size());
    std::_Exit(7);
}
} // namespace std::inline __1

extern "C" {

// `proc.main`, with the depth it runs at: no calls in progress yet (D154).
void unio_main(std::int64_t depth);

// trace: D152, D156
[[noreturn]] void unio_crash(const Site* site) {
    static Buffer message;
    heading(message, "failed", site);
    unio::layout::append_label(message, "what", site->detail);
    unio::layout::append_label(message, "why", "`?? crash` stops the program when the call before it fails.");
    write_message(message);
    std::_Exit(5);
}

// trace: D154
[[noreturn]] void unio_too_deep(const Site* site, std::int64_t depth) {
    static Buffer message;
    heading(message, "check failed", site);
    unio::layout::append_label(message, "what", "calls went too deep");
    Buffer when;
    unio::layout::append_number(when, depth);
    when.append(" calls deep, inside `");
    when.append(site->detail);
    when.append("`");
    unio::layout::append_label(message, "when", std::string_view(when.data(), when.size()));
    unio::layout::append_label(message, "why",
                               "every call still waiting for another needs a little memory, and the room for them is "
                               "limited; a procedure that keeps calling itself without stopping fills it.");
    write_message(message);
    std::_Exit(4);
}

// trace: D6, D104
void unio_show(const Text* text, std::int64_t same_line) {
    output.write(bytes(text), static_cast<std::size_t>(text->length));
    if (same_line == 0) {
        output.write("\n", 1);
    }
}

// trace: D20, D96, D106, D144
Text* unio_ask(const Text* question, const Site* site) {
    output.write(bytes(question), static_cast<std::size_t>(question->length));
    output.flush();
    // The line is gathered in one reused buffer, then copied into a text of exactly its length.
    static char* line = nullptr;
    static std::size_t capacity = 0;
    std::size_t length = 0;
    const bool any = input.read_line([&](const char* data, std::size_t size) {
        if (length + size > capacity) {
            std::size_t grown = capacity < 256 ? 256 : capacity * 2;
            while (grown < length + size) {
                grown *= 2;
            }
            auto* larger = // NOLINT(cppcoreguidelines-owning-memory): the reused line buffer, kept for the whole run
                static_cast<char*>(
                    std::realloc(line, grown)); // NOLINT(cppcoreguidelines-no-malloc,cppcoreguidelines-owning-memory)
            if (larger == nullptr) {
                out_of_memory(site, grown);
            }
            line = larger;
            capacity = grown;
        }
        std::memcpy(line + length, data, size); // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
        length += size;
    });
    if (!any) {
        output.flush();
        std::_Exit(0);
    }
    if (length > 0 && line[length - 1] == '\r') { // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
        --length;
    }
    Text* answer = allocate_text(static_cast<std::int64_t>(length), site);
    if (length > 0) {
        std::memcpy(bytes(answer), line, length);
    }
    if (isatty(0) != 0) {
        output.close_line();
    }
    return answer;
}

void unio_retain(Text* text) {
    if (text->owners >= 0) {
        ++text->owners;
    }
}

void unio_release(Text* text) {
    if (text->owners < 0) {
        return;
    }
    if (--text->owners == 0) {
        free_text(text);
    }
}

// trace: D28
Text* unio_join(const Text* left, const Text* right, const Site* site) {
    Text* joined = allocate_text(left->length + right->length, site);
    std::memcpy(bytes(joined), bytes(left), static_cast<std::size_t>(left->length));
    std::memcpy(bytes(joined) + left->length, bytes(right), // NOLINT(cppcoreguidelines-pro-bounds-pointer-arithmetic)
                static_cast<std::size_t>(right->length));
    return joined;
}

// trace: D22
Text* unio_whole_to_text(std::int64_t value, const Site* site) {
    Buffer digits;
    unio::layout::append_number(digits, value);
    Text* text = allocate_text(static_cast<std::int64_t>(digits.size()), site);
    std::memcpy(bytes(text), digits.data(), digits.size());
    return text;
}

// trace: D95, D143
bool unio_parse_whole(const Text* text, std::int64_t* value) {
    const std::string_view written(bytes(text), static_cast<std::size_t>(text->length));
    std::size_t start = 0;
    std::size_t end = written.size();
    while (start < end && (written[start] == ' ' || written[start] == '\t')) {
        ++start;
    }
    while (end > start && (written[end - 1] == ' ' || written[end - 1] == '\t')) {
        --end;
    }
    bool negative = false;
    if (start < end && (written[start] == '+' || written[start] == '-')) {
        negative = written[start] == '-';
        ++start;
    }
    if (start == end) {
        return false;
    }
    // Built up as a negative number, which reaches one further than a positive one.
    std::int64_t result = 0;
    constexpr std::int64_t lowest = std::numeric_limits<std::int64_t>::min();
    for (std::size_t index = start; index < end; ++index) {
        const char character = written[index];
        if (character < '0' || character > '9') {
            return false;
        }
        const int digit = character - '0';
        if (result < (lowest + digit) / 10) {
            return false;
        }
        result = result * 10 - digit;
    }
    if (!negative) {
        if (result == lowest) {
            return false;
        }
        result = -result;
    }
    *value = result;
    return true;
}

// trace: D26, D56
std::int64_t unio_random(std::int64_t low, std::int64_t high, const Site* site) {
    if (low > high) {
        static Buffer message;
        heading(message, "NotDesignedYet", site);
        unio::layout::append_label(message, "what", "`random` with `from:` above `to:` isn't decided yet");
        write_message(message);
        std::_Exit(3);
    }
    const std::uint64_t span = static_cast<std::uint64_t>(high) - static_cast<std::uint64_t>(low) + 1;
    std::uint64_t drawn = 0;
    if (span == 0) {
        arc4random_buf(&drawn, sizeof drawn);
    } else {
        // Rejecting the top sliver keeps every value equally likely.
        const std::uint64_t limit =
            std::numeric_limits<std::uint64_t>::max() - std::numeric_limits<std::uint64_t>::max() % span;
        while (true) {
            arc4random_buf(&drawn, sizeof drawn);
            if (drawn < limit) {
                break;
            }
        }
        drawn %= span;
    }
    return static_cast<std::int64_t>(static_cast<std::uint64_t>(low) + drawn);
}

// trace: D46, D92, D142
[[noreturn]] void unio_overflow(const Site* site, std::int64_t left, std::int64_t right) {
    static Buffer message;
    heading(message, "check failed", site);
    const auto kind = static_cast<SiteKind>(site->kind);
    bool too_big = false;
    Buffer when;
    switch (kind) {
    case SiteKind::add:
        too_big = right > 0;
        unio::layout::append_number(when, left);
        when.append(" + ");
        unio::layout::append_number(when, right);
        break;
    case SiteKind::subtract:
        too_big = right < 0;
        unio::layout::append_number(when, left);
        when.append(" - ");
        unio::layout::append_number(when, right);
        break;
    case SiteKind::multiply:
        too_big = (left < 0) == (right < 0);
        unio::layout::append_number(when, left);
        when.append(" * ");
        unio::layout::append_number(when, right);
        break;
    default:
        too_big = true;
        when.append("-(");
        unio::layout::append_number(when, right);
        when.append(")");
        break;
    }
    unio::layout::append_label(
        message, "what", too_big ? "the result is too big for a `Whole`" : "the result is too small for a `Whole`");
    unio::layout::append_label(message, "when", std::string_view(when.data(), when.size()));
    unio::layout::append_label(message, "why",
                               "a `Whole` holds whole numbers from -9223372036854775808 to 9223372036854775807.");
    write_message(message);
    std::_Exit(4);
}

} // extern "C"

namespace {

// trace: D154
// The program runs on a thread with a stack large enough for the deepest calls the compiled code allows, so the
// count's own message comes first, never a crash.
void* run_program(void* /*unused*/) {
    unio_main(0);
    output.flush();
    std::_Exit(0);
}

} // namespace

int main() {
    // trace: D134
    std::uint64_t memory = 0;
    std::size_t size = sizeof memory;
    if (sysctlbyname("hw.memsize", &memory, &size, nullptr, 0) != 0) {
        memory = std::uint64_t{4} << 30U;
    }
    memory_cap = memory / 10 * 3;
    pthread_attr_t attributes{};
    pthread_attr_init(&attributes);
    constexpr std::size_t stack = std::size_t{1} << 30U;
    pthread_attr_setstacksize(&attributes, stack);
    pthread_t thread{};
    if (pthread_create(&thread, &attributes, run_program, nullptr) != 0) {
        // Without the large stack, the program still runs; only very deep calls could crash.
        run_program(nullptr);
    }
    pthread_join(thread, nullptr);
    return 0;
}
