// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "messages.h"

#include "layout.h"

#include <algorithm>

namespace unio {

void Messages::refuse(Place place, std::string what, std::string why) {
    Message message;
    message.place = place;
    message.what = std::move(what);
    if (!why.empty()) {
        message.why = std::move(why);
    }
    add(std::move(message));
}

void Messages::fix(Place place, std::string fixed_line, std::string why) {
    Message message;
    message.place = place;
    message.fix = std::move(fixed_line);
    message.why = std::move(why);
    add(std::move(message));
}

// trace: D56, D103
void Messages::not_built_yet(Place place, std::string what) {
    Message message;
    message.kind = MessageKind::not_built_yet;
    message.place = place;
    message.what = std::move(what);
    add(std::move(message));
}

void Messages::not_designed_yet(Place place, std::string what) {
    Message message;
    message.kind = MessageKind::not_designed_yet;
    message.place = place;
    message.what = std::move(what);
    add(std::move(message));
}

bool Messages::has_refusal() const {
    return std::ranges::any_of(list_, [](const Message& message) { return message.kind == MessageKind::refused; });
}

// trace: D56, D148
int Messages::status() const {
    if (has_refusal()) {
        return 1;
    }
    return list_.empty() ? 0 : 3;
}

std::string render(const Message& message, const Sources& sources) {
    std::string out;
    const char* kind = "refused";
    if (message.kind == MessageKind::not_built_yet) {
        kind = "NotBuiltYet";
    } else if (message.kind == MessageKind::not_designed_yet) {
        kind = "NotDesignedYet";
    }
    const LineColumn where = sources.line_column(message.place);
    // trace: D54
    layout::append_heading(out, kind, sources.file(message.place.file).name, where.line, where.column);
    layout::append_source(out, where.line, sources.line_text(message.place.file, where.line), where.column,
                          message.place.length);
    if (message.fix) {
        layout::append_label(out, "fix", "`" + *message.fix + "`");
    } else if (message.what) {
        layout::append_label(out, "what", *message.what);
    }
    if (message.because) {
        layout::append_label(out, "because", message.because->sentence);
        if (message.because->place) {
            const Place cause = *message.because->place;
            const LineColumn cause_where = sources.line_column(cause);
            layout::append_source(out, cause_where.line, sources.line_text(cause.file, cause_where.line),
                                  cause_where.column, cause.length);
        }
    }
    if (message.why) {
        layout::append_label(out, "why", *message.why);
    }
    return out;
}

std::string Messages::render(const Sources& sources) const {
    std::string out;
    for (const Message& message : list_) {
        out += unio::render(message, sources);
    }
    return out;
}

} // namespace unio
