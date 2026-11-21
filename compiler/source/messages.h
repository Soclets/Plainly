// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Messages about a program, in D54's layout, and the exit status they lead to (D56, D148).

#pragma once

#include "source.h"

#include <optional>
#include <string>
#include <vector>

namespace unio {

enum class MessageKind : std::uint8_t { refused, not_built_yet, not_designed_yet };

struct Because {
    std::string sentence;
    std::optional<Place> place; // the line shown under it, with `^` under the cause
};

struct Message {
    MessageKind kind = MessageKind::refused;
    Place place;
    std::optional<std::string> fix; // the whole corrected line (D84), shown in backticks
    std::optional<std::string> what;
    std::optional<Because> because;
    std::optional<std::string> why;
    // A message that knock-on sorting (D55, D83) may hide when an earlier fix makes it disappear.
    bool from_recovery = false;
};

class Messages {
public:
    void add(Message message) { list_.push_back(std::move(message)); }
    void refuse(Place place, std::string what, std::string why);
    void fix(Place place, std::string fixed_line, std::string why);
    void not_built_yet(Place place, std::string what);
    void not_designed_yet(Place place, std::string what);

    [[nodiscard]] bool empty() const { return list_.empty(); }
    [[nodiscard]] bool has_refusal() const;
    [[nodiscard]] const std::vector<Message>& list() const { return list_; }
    std::vector<Message>& list() { return list_; }
    // Status 1 for refusals, else 3 for NotBuiltYet or NotDesignedYet alone, else 0 (D56, D148).
    [[nodiscard]] int status() const;
    [[nodiscard]] std::string render(const Sources& sources) const;

private:
    std::vector<Message> list_;
};

std::string render(const Message& message, const Sources& sources);

} // namespace unio
