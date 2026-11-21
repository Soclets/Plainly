(* The parser: reads a Unio file's tokens into the core's syntax tree (core/Syntax.v).

   It refuses what's about how a program is written: `;` and `:` (D7, D8), closers (D8), indentation (D10), text in
   single quotes (D21), `=` in a condition (D14), chains of comparisons (D81), names (D99), and blank lines (D97). What a
   program means is the core's to judge (core/Checker.v).

   After a mistake it carries on with the next statement (D55), and a part it couldn't read becomes [Broken], which the
   checker passes over silently. When an opening line is too broken to tell what was meant, checking stops there
   (D55, rule 5). *)

open Lexer
module M = Messages

let keywords =
  [ "proc"; "end"; "let"; "var"; "if"; "else"; "at_least"; "at_most"; "while"; "break"; "true"; "false"; "and";
    "or"; "not"; "be"; "ignore"; "crash"; "otherwise"; "case"; "instead"; "labels"; "as"; "extern"; "returns";
    "return"; "fam"; "div"; "rem"; "handler"; "include"; "comment"; "fails" ]

let is_keyword word = List.mem word keywords

type state = {
  tokens : token array;
  lines : Lexer.line array;
  mutable index : int;
  mutable problems : M.message list;
  mutable stopped : bool;  (** checking stopped at a broken opening line (D55, rule 5) *)
  mutable statement_line : int;  (** the line the statement being read starts on *)
  mutable spread_reported : bool;
  mutable closers : string list;  (** the closers of the blocks open now, innermost first *)
  mutable last_code_line : int;  (** the last line whose indentation was checked *)
}

(* Every way a statement or expression can fail to read; the problem has already been recorded. *)
type 'a parsed = ('a, unit) result

let ( let* ) = Result.bind

let line_text st number =
  if number >= 1 && number <= Array.length st.lines then st.lines.(number - 1).text else ""

let report st m = st.problems <- m :: st.problems
let peek st = if st.index < Array.length st.tokens then Some st.tokens.(st.index) else None
let peek_ahead st k =
  if st.index + k < Array.length st.tokens then Some st.tokens.(st.index + k) else None
let previous st = if st.index > 0 then Some st.tokens.(st.index - 1) else None

(* Takes the next token, noting a statement that runs onto another line: how its later lines are indented isn't
   decided (chapter 2). *)
let advance st =
  match peek st with
  | None -> ()
  | Some t ->
      if t.line <> st.statement_line && not st.spread_reported then begin
        st.spread_reported <- true;
        report st
          (M.message M.Not_designed_yet (M.point t.line t.column t.width)
             ~what:"how a statement's later lines are indented isn't decided yet")
      end;
      st.index <- st.index + 1

let is_symbol s t = match t.kind with Symbol s' -> s = s' | _ -> false
let is_word w t = match t.kind with Word w' -> w = w' | _ -> false
let next_is st test = match peek st with Some t -> test t | None -> false

type span = { line : int; first : int; stop : int }

let span_of (t : token) = { line = t.line; first = t.column; stop = t.column + t.width }

let join st a b =
  if a.line = b.line then { a with stop = b.stop }
  else { a with stop = String.length (line_text st a.line) + 1 }

let place_of s = { Core.place_line = s.line; place_column = s.first; place_width = s.stop - s.first }
let place_of_token t = place_of (span_of t)

(* The end of the last token taken, where a missing `;` or `:` goes. *)
let after_previous st =
  match previous st with
  | Some t -> (t.line, t.column + t.width)
  | None -> (1, 1)

(* trace: D92. The parser's own three messages, with no `why:`. *)
let expected st what =
  match peek st with
  | Some t when t.line = st.statement_line || st.statement_line = 0 ->
      report st
        (M.message M.Refused (M.point t.line t.column t.width)
           ~what:(Printf.sprintf "%s should be here, not `%s`" what (describe t.kind)))
  | Some _ ->
      let line, column = after_previous st in
      report st (M.message M.Refused (M.point line column 1) ~what:(Printf.sprintf "the line ends where %s should be" what))
  | None ->
      let line, column = after_previous st in
      report st (M.message M.Refused (M.point line column 1) ~what:(Printf.sprintf "the file ends where %s should be" what))

let not_built st (t : token) what =
  report st (M.message M.Not_built_yet (M.point t.line t.column t.width) ~what:(what ^ " aren't built yet"))

let not_built_is st (t : token) what =
  report st (M.message M.Not_built_yet (M.point t.line t.column t.width) ~what:(what ^ " isn't built yet"))

let not_designed st (t : token) what =
  report st (M.message M.Not_designed_yet (M.point t.line t.column t.width) ~what:(what ^ " isn't decided yet"))

(* Skips the rest of a statement that couldn't be read: up to its `;`, or the end of its line. *)
let skip_statement st =
  let rec go line =
    match peek st with
    | None -> ()
    | Some t when is_symbol ";" t -> st.index <- st.index + 1
    | Some t when t.line <> line -> ()
    | Some _ ->
        st.index <- st.index + 1;
        go line
  in
  match peek st with Some t -> go t.line | None -> ()

(* trace: D99, D102. A name: lowercase letters and digits, starting with a letter, with `_` only between words. *)
let check_name st (t : token) written name =
  let length = String.length name in
  let capital = String.exists (fun c -> c >= 'A' && c <= 'Z') name in
  if length = 0 then not_designed st t "a `$` with no name after it"
  else if capital then
    report st
      (M.message M.Refused (M.point t.line t.column t.width)
         ~what:(Printf.sprintf "`%s` has a capital letter, but names have none" written)
         ~why:"a name is lowercase letters and digits, starting with a letter, with `_` only between words.")
  else
    (* trace: D142 *)
    let problem =
      if is_digit name.[0] then Some "starts with a digit, but names start with a letter"
      else if name.[0] = '_' then Some "starts with `_`, but names start with a letter"
      else if name.[length - 1] = '_' then Some "ends with `_`, but `_` only goes between words"
      else
        let rec doubled i = i + 1 < length && ((name.[i] = '_' && name.[i + 1] = '_') || doubled (i + 1)) in
        if doubled 0 then Some "has two `_` in a row, but `_` only goes between words" else None
    in
    match problem with
    | Some problem ->
        report st
          (M.message M.Refused (M.point t.line t.column t.width)
             ~what:(Printf.sprintf "`%s` %s" written problem)
             ~why:"a name is lowercase letters and digits, starting with a letter, with `_` only between words.")
    | None -> ()

(* ---------------------------------------------------------------------------------------------------------------- *)
(* Text *)

(* trace: D30. Text's value: `\` makes the next character literal. *)
let plain_value raw =
  let buffer = Buffer.create (String.length raw) in
  let rec go i =
    if i < String.length raw then
      if raw.[i] = '\\' && i + 1 < String.length raw then begin
        Buffer.add_char buffer raw.[i + 1];
        go (i + 2)
      end
      else begin
        Buffer.add_char buffer raw.[i];
        go (i + 1)
      end
  in
  go 0;
  Buffer.contents buffer

let all_word_characters s = String.length s > 0 && String.for_all is_word_character s

(* trace: D29, D30, D133. Text's pieces, as `.fmt` reads it; [column] is where the text's first character is. *)
let pieces_of raw line column =
  let place_at i width = { Core.place_line = line; place_column = column + i; place_width = width } in
  let buffer = Buffer.create (String.length raw) in
  let flush pieces =
    if Buffer.length buffer = 0 then pieces
    else begin
      let text = Buffer.contents buffer in
      Buffer.clear buffer;
      Core.Piece_text text :: pieces
    end
  in
  let rec go i pieces =
    if i >= String.length raw then List.rev (flush pieces)
    else if raw.[i] = '\\' && i + 1 < String.length raw then begin
      Buffer.add_char buffer raw.[i + 1];
      go (i + 2) pieces
    end
    else if raw.[i] = '{' then
      match String.index_from_opt raw (i + 1) '}' with
      | None -> List.rev (Core.Piece_unclosed (place_at i 1) :: flush pieces)
      | Some j ->
          let inside = String.sub raw (i + 1) (j - i - 1) in
          let piece =
            if inside = "" then Core.Piece_other (place_at i 2)
            else if inside.[0] = '$' && all_word_characters (String.sub inside 1 (String.length inside - 1)) then
              Core.Piece_variable (String.sub inside 1 (String.length inside - 1), place_at (i + 1) (j - i - 1))
            else if all_word_characters inside && is_letter inside.[0] then
              Core.Piece_bare_name (inside, place_at (i + 1) (j - i - 1))
            else Core.Piece_other (place_at (i + 1) (j - i - 1))
          in
          go (j + 1) (piece :: flush pieces)
    else begin
      Buffer.add_char buffer raw.[i];
      go (i + 1) pieces
    end
  in
  go 0 []

(* ---------------------------------------------------------------------------------------------------------------- *)
(* Expressions *)

let comparison_operator t =
  match t.kind with
  | Symbol "==" -> Some Core.Equal
  | Symbol "!=" -> Some Core.Not_equal
  | Symbol "<" -> Some Core.Less
  | Symbol ">" -> Some Core.Greater
  | Word "at_least" -> Some Core.At_least
  | Word "at_most" -> Some Core.At_most
  | _ -> None

let source_of st s = String.sub (line_text st s.line) (s.first - 1) (s.stop - s.first)

(* A callee: a word and its narrowings (D18), as in `show_msg.fmt.same_line`. *)
let parse_callee st (word : token) name =
  advance st;
  let rec narrowings acc last =
    match (peek st, peek_ahead st 1) with
    | Some dot, Some ({ kind = Word n; _ } as t) when is_symbol "." dot ->
        advance st;
        advance st;
        narrowings ((n, place_of_token t) :: acc) (span_of t)
    | _ -> (List.rev acc, last)
  in
  let narrowings, last = narrowings [] (span_of word) in
  ({ Core.callee_word = name; callee_word_at = place_of_token word; callee_narrowings = narrowings }, join st (span_of word) last)

let rec parse_expression st : (Core.expression * span) parsed = parse_slide st

(* trace: D5, D73. `|>` comes last, so a slide takes everything on its left. *)
and parse_slide st =
  let* start, start_span = parse_or st in
  if next_is st (is_symbol "|>") then
    let rec steps acc last =
      match peek st with
      | Some t when is_symbol "|>" t -> (
          advance st;
          match peek st with
          | Some ({ kind = Word name; _ } as word) when not (is_keyword name && name <> "be") ->
              let callee, callee_span = parse_callee st word name in
              let* step =
                if next_is st (is_symbol "(") then
                  let* given, close = parse_arguments st in
                  (match given with
                  | Core.No_arguments ->
                      report st
                        (M.message M.Refused
                           (M.point callee_span.line callee_span.stop (close.stop - callee_span.stop))
                           ~fix:
                             (M.replace_at (line_text st callee_span.line) callee_span.stop
                                (close.stop - callee_span.stop) "")
                           ~why:"a step with nothing else to pass is written bare, since the slide gives it its value.")
                  | _ -> ());
                  Ok (callee, true, given, join st callee_span close)
                else Ok (callee, false, Core.No_arguments, callee_span)
              in
              let _, _, _, step_span = step in
              (match peek st with
              | Some t when Option.is_some (binary_operator t) || is_symbol "??" t ->
                  not_built st t "an operator after a slide's step, refused with brackets as the fix (D81, D128),";
                  Error ()
              | _ -> steps (step :: acc) step_span)
          | _ ->
              expected st "a procedure";
              Error ())
      | _ ->
          let path =
            List.fold_left
              (fun rest (callee, bracketed, given, s) -> Core.Step (callee, bracketed, given, place_of s, rest))
              Core.No_steps acc
          in
          let whole = join st start_span last in
          Ok (Core.Slide (start, path, place_of whole), whole)
    in
    steps [] start_span
  else Ok (start, start_span)

and binary_operator t =
  match t.kind with
  | Symbol "+" -> Some Core.Add
  | Symbol "-" -> Some Core.Subtract
  | Symbol "*" -> Some Core.Multiply
  | Symbol "&" -> Some Core.Join
  | Word "and" -> Some Core.And
  | Word "or" -> Some Core.Or
  | _ -> comparison_operator t

and left_to_right st operators next =
  let* first, first_span = next st in
  let rec loop left left_span =
    match peek st with
    | Some t -> (
        match List.assoc_opt t.kind operators with
        | Some operator ->
            advance st;
            let* right, right_span = next st in
            let whole = join st left_span right_span in
            loop (Core.Binary (operator, left, right, place_of_token t, place_of whole)) whole
        | None -> Ok (left, left_span))
    | None -> Ok (left, left_span)
  in
  loop first first_span

(* trace: D34. `not` first, then `and`, then `or`, with comparisons before all three. *)
and parse_or st = left_to_right st [ (Word "or", Core.Or) ] parse_and
and parse_and st = left_to_right st [ (Word "and", Core.And) ] parse_not

and parse_not st =
  match peek st with
  | Some t when is_word "not" t ->
      advance st;
      let* operand, operand_span = parse_not st in
      let whole = join st (span_of t) operand_span in
      Ok (Core.Not (operand, place_of whole), whole)
  | _ -> parse_comparison st

(* trace: D17, D81, D102. One comparison; two in a row are refused, with `and` as the fix. *)
and parse_comparison st =
  let* a, a_span = parse_additive st in
  match peek st with
  | Some t -> (
      match comparison_operator t with
      | None -> Ok (a, a_span)
      | Some operator -> (
          advance st;
          let* b, b_span = parse_additive st in
          let whole = join st a_span b_span in
          match peek st with
          | Some t2 when Option.is_some (comparison_operator t2) ->
              advance st;
              let* _, c_span = parse_additive st in
              if a_span.line = c_span.line then begin
                let text = line_text st a_span.line in
                let middle = source_of st b_span in
                let first_comparison = String.sub text (a_span.first - 1) (b_span.stop - a_span.first) in
                let second_comparison = middle ^ String.sub text (b_span.stop - 1) (c_span.stop - b_span.stop) in
                report st
                  (M.message M.Refused (M.point t2.line t2.column t2.width)
                     ~fix:
                       (M.replace_at text a_span.first (c_span.stop - a_span.first)
                          (first_comparison ^ " and " ^ second_comparison))
                     ~why:
                       "a comparison sets two values against each other; `and` joins two comparisons, naming the \
                        middle value twice.")
              end;
              Error ()
          | _ -> Ok (Core.Binary (operator, a, b, place_of_token t, place_of whole), whole)))
  | None -> Ok (a, a_span)

(* trace: D27, D28, D72. `+`, `-` and `&` read left to right; `&` beside maths is the checker's to refuse. *)
and parse_additive st =
  left_to_right st [ (Symbol "+", Core.Add); (Symbol "-", Core.Subtract); (Symbol "&", Core.Join) ] parse_multiplicative

and parse_multiplicative st =
  let* first, first_span = parse_unary st in
  let rec loop left left_span =
    match peek st with
    | Some t when is_symbol "*" t ->
        advance st;
        let* right, right_span = parse_unary st in
        let whole = join st left_span right_span in
        loop (Core.Binary (Core.Multiply, left, right, place_of_token t, place_of whole)) whole
    | Some t when is_symbol "/" t || is_word "div" t || is_word "rem" t ->
        not_built st t "dividing and remainders, `/`, `div` and `rem` (D70, D71),";
        Error ()
    | _ -> Ok (left, left_span)
  in
  loop first first_span

(* trace: D27, D60. A `-` with nothing before it but an operator or a bracket negates. *)
and parse_unary st =
  match peek st with
  | Some t when is_symbol "-" t ->
      advance st;
      let* operand, operand_span = parse_unary st in
      let whole = join st (span_of t) operand_span in
      Ok (Core.Negate (operand, place_of whole), whole)
  | _ -> parse_postfix st

(* trace: D23, D81. `??` belongs to the call right before it, and takes a single value after it. *)
and parse_postfix st =
  let* value, value_span = parse_primary st in
  match peek st with
  | Some t when is_symbol "??" t -> (
      advance st;
      match peek st with
      | Some u when is_word "ignore" u || is_word "crash" u ->
          not_built st u "`?? ignore` and `?? crash` (D23, D121)";
          Error ()
      | _ ->
          let* fallback, fallback_span =
            match peek st with
            | Some m when is_symbol "-" m ->
                advance st;
                let* operand, operand_span = parse_primary st in
                let whole = join st (span_of m) operand_span in
                Ok (Core.Negate (operand, place_of whole), whole)
            | _ -> parse_primary st
          in
          let whole = join st value_span fallback_span in
          Ok (Core.Fallback (value, fallback, place_of_token t, place_of whole), whole))
  | Some t when is_symbol "?" t ->
      not_built st t "passing a failure up with `?` (D23)";
      Error ()
  | Some t when is_symbol "." t ->
      not_built st t "methods and facts after a value, as in `$list.len` (D77, D79),";
      Error ()
  | _ -> Ok (value, value_span)

and parse_primary st =
  match peek st with
  | None ->
      expected st "a value";
      Error ()
  | Some t -> (
      match t.kind with
      | Number digits ->
          advance st;
          Ok (Core.Whole_literal (digits, place_of_token t), span_of t)
      | Text raw ->
          advance st;
          Ok (Core.Text_literal (plain_value raw, pieces_of raw t.line (t.column + 1), place_of_token t), span_of t)
      | Single_quoted raw ->
          advance st;
          report st
            (M.message M.Refused (M.point t.line t.column t.width)
               ~fix:(M.replace_at (line_text st t.line) t.column t.width ("\"" ^ raw ^ "\""))
               ~why:"text is written in double quotes, never single ones like `'hi'`.");
          Ok (Core.Broken (place_of_token t), span_of t)
      | Unclosed_text _ ->
          advance st;
          let line, column = (t.line, t.column + t.width) in
          report st (M.message M.Refused (M.point line column 1) ~what:"the line ends where `\"` should be");
          Error ()
      | Dollar name ->
          advance st;
          if next_is st (is_symbol "(") then begin
            report st
              (M.message M.Refused (M.point t.line t.column t.width)
                 ~what:(Printf.sprintf "`$%s` is a variable, not a procedure" name)
                 ~why:"only a procedure can be called.");
            Error ()
          end
          else Ok (Core.Variable_value (name, place_of_token t), span_of t)
      | Word "true" ->
          advance st;
          Ok (Core.Bool_literal (true, place_of_token t), span_of t)
      | Word "false" ->
          advance st;
          Ok (Core.Bool_literal (false, place_of_token t), span_of t)
      | Word name when (not (is_keyword name)) || name = "be" ->
          let callee, callee_span = parse_callee st t name in
          if next_is st (is_symbol "(") then
            let* given, close = parse_arguments st in
            let whole = join st callee_span close in
            Ok (Core.Call (callee, given, place_of whole), whole)
          else Ok (Core.Bare_word (name, place_of callee_span), callee_span)
      | Symbol "(" ->
          advance st;
          let* inner, _ = parse_expression st in
          let* close = expect_closing st ")" in
          let whole = join st (span_of t) close in
          Ok (Core.Bracketed (inner, place_of whole), whole)
      | Symbol "[" ->
          not_built st t "lists, `List<Type>` and `[ ]` (D75),";
          Error ()
      | Symbol "::" ->
          not_built st t "packages, named with `::` (D64),";
          Error ()
      | _ ->
          expected st "a value";
          Error ())

and expect_closing st symbol =
  match peek st with
  | Some t when is_symbol symbol t ->
      advance st;
      Ok (span_of t)
  | _ ->
      expected st ("`" ^ symbol ^ "`");
      Error ()

(* A call's arguments, each with its label if it's written with one (D24). Gives the closing bracket's place. *)
and parse_arguments st =
  advance st;
  if next_is st (is_symbol ")") then
    match peek st with
    | Some t ->
        advance st;
        Ok (Core.No_arguments, span_of t)
    | None -> Error ()
  else
    let rec loop acc =
      let label =
        match (peek st, peek_ahead st 1) with
        | Some ({ kind = Word w; _ } as t), Some colon when is_symbol ":" colon && not (is_keyword w) ->
            advance st;
            advance st;
            Some (w, place_of_token t)
        | _ -> None
      in
      let* value, _ = parse_expression st in
      match peek st with
      | Some t when is_symbol "," t ->
          advance st;
          loop ((label, value) :: acc)
      | _ ->
          let* close = expect_closing st ")" in
          let given =
            List.fold_left (fun rest (label, value) -> Core.Argument (label, value, rest)) Core.No_arguments
              ((label, value) :: acc)
          in
          Ok (given, close)
    in
    loop []

(* ---------------------------------------------------------------------------------------------------------------- *)
(* Lines and blocks *)

(* trace: D10. Checks that a statement, opening line or closer starts its line, indented [depth] levels of 4 spaces. *)
let begin_line st (t : token) depth =
  st.statement_line <- t.line;
  st.spread_reported <- false;
  let text = line_text st t.line in
  let first_on_line =
    match st.lines.(t.line - 1).tokens with first :: _ -> first.column = t.column | [] -> false
  in
  if not first_on_line then
    (* trace: D146 *)
    report st
      (M.message M.Refused
         (M.point t.line t.column (String.length text - t.column + 1))
         ~what:"this line has two statements" ~why:"each statement goes on a line of its own.")
  else begin
    (* Lines holding only a comment, since the last line checked. *)
    for number = st.last_code_line + 1 to t.line - 1 do
      let l = st.lines.(number - 1) in
      if l.tokens = [] && l.comment then begin
        (* trace: D145. Indented like the line after it, or one level in right before a closer or `else`. *)
        let content = String.trim l.text in
        let indent = String.length l.text - String.length content in
        let levels = if is_word "end" t || is_word "else" t then depth + 1 else depth in
        let wanted = String.make (4 * levels) ' ' in
        if String.sub l.text 0 indent <> wanted then
          report st
            (M.message M.Refused (M.point number 1 indent) ~fix:(wanted ^ content)
               ~why:"each level of indentation is 4 spaces, and tabs aren't used.")
      end
    done;
    st.last_code_line <- t.line;
    let indentation = String.sub text 0 (t.column - 1) in
    if indentation <> String.make (4 * depth) ' ' then
      report st
        (M.message M.Refused
           (M.point t.line 1 (String.length indentation))
           ~fix:(String.make (4 * depth) ' ' ^ String.sub text (t.column - 1) (String.length text - t.column + 1))
           ~why:"each level of indentation is 4 spaces, and tabs aren't used.")
  end

(* trace: D7. Every statement ends with `;`; a missing one is refused with the place it goes as the fix. *)
let expect_semicolon st =
  match peek st with
  | Some t when is_symbol ";" t -> advance st
  | Some t when t.line = st.statement_line ->
      expected st "`;`";
      skip_statement st
  | _ ->
      let line, column = after_previous st in
      report st
        (M.message M.Refused (M.point line column 1)
           ~fix:(M.insert_at (line_text st line) column ";")
           ~why:"every statement ends with `;`.")

(* trace: D8, D55. An opening line ends with `:`. Missing only that, it's clear what was meant, and checking carries
   on into the body; anything else stops checking there. *)
let expect_opener_colon st =
  match peek st with
  | Some t when is_symbol ":" t && t.line = st.statement_line -> (
      advance st;
      match peek st with
      | Some u when u.line = t.line ->
          expected st "the end of the line";
          st.stopped <- true
      | _ -> ())
  | Some t when t.line = st.statement_line ->
      expected st "`:`";
      st.stopped <- true
  | _ ->
      let line, column = after_previous st in
      report st
        (M.message M.Refused (M.point line column 1)
           ~fix:(M.insert_at (line_text st line) column ":")
           ~why:"a block's opening line ends with `:`.")

(* The words of a closer, as in ["end"; "proc"; "."; "main"]. *)
let closer_words st =
  let rec go line acc =
    match peek_ahead st (List.length acc) with
    | Some t when t.line = line && not (is_symbol ";" t) -> go line (describe t.kind :: acc)
    | _ -> List.rev acc
  in
  match peek st with Some t -> go t.line [] | None -> []

let closer_text words =
  match words with
  | "end" :: "proc" :: "." :: rest -> "end proc." ^ String.concat "" rest
  | _ -> String.concat " " words

(* trace: D8, D85, D87. Reads the closer [wanted] at [depth]. *)
let parse_closer st depth wanted =
  match peek st with
  | None ->
      let last = Array.length st.lines in
      report st
        (M.message M.Refused
           (M.point last (String.length (line_text st last) + 1) 1)
           ~what:(Printf.sprintf "the file ends before `%s`" wanted)
           ~why:"every block ends with `end` and what it closes.")
  | Some t ->
      let found = closer_text (closer_words st) in
      if found = wanted then begin
        begin_line st t depth;
        List.iter (fun _ -> advance st) (closer_words st);
        match peek st with
        | Some semicolon when is_symbol ";" semicolon && semicolon.line = t.line ->
            advance st;
            report st
              (M.message M.Refused (M.point semicolon.line semicolon.column 1)
                 ~fix:(M.replace_at (line_text st t.line) semicolon.column 1 "")
                 ~why:"a closer stands alone, with no `;`.")
        | _ -> ()
      end
      else if List.mem found st.closers then
        (* The closer of a block further out: this block's own closer is missing. *)
        report st
          (M.message M.Refused (M.point t.line t.column t.width)
             ~what:(Printf.sprintf "`%s` is missing before this line" wanted)
             ~why:"every block ends with `end` and what it closes.")
      else begin
        begin_line st t depth;
        let text = line_text st t.line in
        report st
          (M.message M.Refused
             (M.point t.line t.column (String.length text - t.column + 1))
             ~fix:(String.make (4 * depth) ' ' ^ wanted)
             ~why:"a closer is `end` and what it closes.");
        let rec skip () =
          match peek st with
          | Some u when u.line = t.line ->
              advance st;
              skip ()
          | _ -> ()
        in
        skip ()
      end

let parse_type st : Core.type0 parsed =
  match peek st with
  | Some ({ kind = Word "Whole"; _ }) ->
      advance st;
      Ok Core.Whole
  | Some ({ kind = Word "Bool"; _ }) ->
      advance st;
      Ok Core.Bool
  | Some ({ kind = Word "Text"; _ }) ->
      advance st;
      Ok Core.Text
  | Some ({ kind = Word "List"; _ } as t) ->
      not_built st t "lists, `List<Type>` and `[ ]` (D75),";
      Error ()
  | Some ({ kind = Word w; _ } as t) when String.length w > 0 && w.[0] >= 'A' && w.[0] <= 'Z' ->
      not_designed st t (Printf.sprintf "a type named `%s`" w);
      Error ()
  | _ ->
      expected st "a type";
      Error ()

let statement_span st (t : token) =
  let last =
    match previous st with Some p when p.line = t.line -> p.column + p.width | _ -> t.column + t.width
  in
  { line = t.line; first = t.column; stop = last }

(* trace: D13. `let` or `var`, the name, `:`, the type, `=` and the first value. *)
let parse_declaration st (keyword : token) changeable : Core.statement parsed =
  advance st;
  match peek st with
  | Some ({ kind = Dollar name; _ } as name_token) -> (
      advance st;
      check_name st name_token ("$" ^ name) name;
      let* written =
        match peek st with
        | Some t when is_symbol ":" t ->
            advance st;
            parse_type st
        | Some ({ kind = Word w; _ } as t) when w = "Whole" || w = "Bool" || w = "Text" ->
            report st
              (M.message M.Refused
                 (M.point t.line (name_token.column + name_token.width) 1)
                 ~fix:(M.insert_at (line_text st t.line) (name_token.column + name_token.width) ":")
                 ~why:"a declaration writes `:` between the variable's name and its type.");
            parse_type st
        | Some t when is_symbol "=" t ->
            report st
              (M.message M.Refused (M.point name_token.line name_token.column name_token.width)
                 ~what:(Printf.sprintf "`$%s` needs its type, written after a `:`" name)
                 ~why:"a declaration always writes the variable's type.");
            Error ()
        | _ ->
            expected st "`:`";
            Error ()
      in
      let* () =
        match peek st with
        | Some t when is_symbol "=" t ->
            advance st;
            Ok ()
        | Some t when is_symbol ";" t ->
            not_designed st t "declaring a variable without a first value";
            Error ()
        | Some t when t.line = keyword.line ->
            let column = match previous st with Some p -> p.column + p.width | None -> t.column in
            report st
              (M.message M.Refused (M.point t.line column 1)
                 ~fix:(M.insert_at (line_text st t.line) column " =")
                 ~why:"a declaration gives its variable a first value after `=`.");
            Ok ()
        | _ ->
            expected st "`=`";
            Error ()
      in
      let value =
        match parse_expression st with
        | Ok (value, _) -> value
        | Error () -> Core.Broken (place_of_token keyword)
      in
      (match value with Core.Broken _ -> skip_statement st | _ -> expect_semicolon st);
      Ok
        (Core.Declare
           ( changeable, name, place_of_token name_token, place_of_token keyword, written, value,
             place_of (statement_span st keyword) )))
  | Some ({ kind = Word w; _ } as t) when not (is_keyword w) ->
      not_designed st t "the message for a variable written without its `$`";
      Error ()
  | _ ->
      expected st "a variable's name";
      Error ()

(* The condition of `if`, `else if` or `while`: `=` in it is refused, with `==` as the fix (D14). *)
let parse_condition st : Core.expression =
  match parse_expression st with
  | Ok (condition, _) -> (
      match peek st with
      | Some t when is_symbol "=" t && t.line = st.statement_line ->
          report st
            (M.message M.Refused (M.point t.line t.column 1)
               ~fix:(M.replace_at (line_text st t.line) t.column 1 "==")
               ~why:"`=` gives a variable a value, and is never part of a condition; `==` compares.");
          advance st;
          let _ = parse_expression st in
          Core.Broken (place_of_token t)
      | _ -> condition)
  | Error () ->
      let rec skip () =
        match peek st with
        | Some t when t.line = st.statement_line && not (is_symbol ":" t) ->
            advance st;
            skip ()
        | _ -> ()
      in
      skip ();
      Core.Broken { Core.place_line = st.statement_line; place_column = 1; place_width = 1 }

let rec parse_block st depth : Core.block =
  let rec statements acc =
    if st.stopped then List.rev acc
    else
      match peek st with
      | None -> List.rev acc
      | Some t when is_word "end" t || is_word "else" t -> List.rev acc
      | Some t -> (
          begin_line st t depth;
          match parse_statement st t depth with
          | Ok statement -> statements (statement :: acc)
          | Error () ->
              skip_statement st;
              statements acc)
  in
  List.fold_right (fun statement rest -> Core.Then (statement, rest)) (statements []) Core.Empty

and parse_statement st (t : token) depth : Core.statement parsed =
  match t.kind with
  | Word "let" -> parse_declaration st t false
  | Word "var" -> parse_declaration st t true
  | Word "if" -> parse_if st t depth
  | Word "while" ->
      advance st;
      let condition = parse_condition st in
      expect_opener_colon st;
      let at = place_of (statement_span st t) in
      st.closers <- "end while" :: st.closers;
      let body = parse_block st (depth + 1) in
      st.closers <- (match st.closers with _ :: rest -> rest | [] -> []);
      if not st.stopped then parse_closer st depth "end while";
      Ok (Core.While (condition, body, at))
  | Word "break" ->
      advance st;
      let at = place_of (statement_span st t) in
      expect_semicolon st;
      Ok (Core.Break at)
  | Word "return" ->
      advance st;
      if next_is st (is_symbol ";") then begin
        let at = place_of (statement_span st t) in
        advance st;
        Ok (Core.Return (None, at))
      end
      else
        let* value, _ = parse_expression st in
        let at = place_of (statement_span st t) in
        expect_semicolon st;
        Ok (Core.Return (Some value, at))
  | Word "labels" -> parse_labels st t
  | Word ("otherwise" | "case" | "instead" | "include") ->
      not_built_is st t "handling failures with `otherwise:` (D40, D43)";
      Error ()
  | Word "comment" ->
      not_built st t "block comments, `comment name:` (D74),";
      st.stopped <- true;
      Error ()
  | Dollar name when (match peek_ahead st 1 with Some u -> is_symbol "=" u | None -> false) ->
      advance st;
      let operator = peek st in
      advance st;
      let* value, _ = parse_expression st in
      let at = place_of (statement_span st t) in
      expect_semicolon st;
      let operator_at = match operator with Some o -> place_of_token o | None -> place_of_token t in
      Ok (Core.Assign (name, place_of_token t, value, operator_at, at))
  | _ ->
      let* action, _ = parse_expression st in
      let at = place_of (statement_span st t) in
      expect_semicolon st;
      Ok (Core.Run (action, at))

(* trace: D16, D102. `if`, any number of `else if`, at most one `else:`, and one `end if`. *)
and parse_if st (t : token) depth =
  advance st;
  let condition = parse_condition st in
  expect_opener_colon st;
  let at = place_of (statement_span st t) in
  st.closers <- "end if" :: st.closers;
  let then_part = parse_block st (depth + 1) in
  let rec rest () =
    if st.stopped then Core.No_else
    else
      match peek st with
      | Some e when is_word "else" e -> (
          begin_line st e depth;
          advance st;
          match peek st with
          | Some i when is_word "if" i ->
              advance st;
              let condition = parse_condition st in
              expect_opener_colon st;
              let at = place_of (statement_span st e) in
              let body = parse_block st (depth + 1) in
              let others = rest () in
              Core.Else_if (condition, body, others, at)
          | _ ->
              expect_opener_colon st;
              let at = place_of (statement_span st e) in
              let body = parse_block st (depth + 1) in
              (match peek st with
              | Some extra when is_word "else" extra ->
                  report st
                    (M.message M.Refused (M.point extra.line extra.column extra.width)
                       ~what:"nothing can follow an `if`'s `else:` part but `end if`"
                       ~why:"an `if` has at most one `else:`, and it comes last.");
                  st.stopped <- true
              | _ -> ());
              Core.Else (body, at))
      | _ -> Core.No_else
  in
  let otherwise = rest () in
  st.closers <- (match st.closers with _ :: rest -> rest | [] -> []);
  if not st.stopped then parse_closer st depth "end if";
  Ok (Core.If (condition, then_part, otherwise, at))

(* trace: D25. `labels $parameter as label, ...;` *)
and parse_labels st (t : token) =
  advance st;
  let rec pairs acc =
    match (peek st, peek_ahead st 1, peek_ahead st 2) with
    | Some ({ kind = Dollar parameter; _ } as p), Some a, Some ({ kind = Word label; _ } as l) when is_word "as" a -> (
        advance st;
        advance st;
        advance st;
        check_name st l label label;
        let pair =
          { Core.pair_parameter = parameter; pair_parameter_at = place_of_token p; pair_label = label;
            pair_label_at = place_of_token l }
        in
        match peek st with
        | Some c when is_symbol "," c ->
            advance st;
            pairs (pair :: acc)
        | _ -> Ok (List.rev (pair :: acc)))
    | _ ->
        expected st "a parameter and its label, as in `$low as from`";
        Error ()
  in
  let* found = pairs [] in
  let at = place_of (statement_span st t) in
  expect_semicolon st;
  Ok (Core.Labels (found, at))

(* trace: D4, D88, D126. A definition's parameters, each its name, `:` and its type. *)
let parse_parameters st (open_bracket : token) : Core.parameter list parsed =
  advance st;
  match peek st with
  | Some close when is_symbol ")" close ->
      advance st;
      report st
        (M.message M.Refused
           (M.point open_bracket.line open_bracket.column 2)
           ~fix:(M.replace_at (line_text st open_bracket.line) open_bracket.column 2 "")
           ~why:"a definition writes `( )` only when it has parameters.");
      Ok []
  | Some ({ kind = Word w; _ } as t) when String.length w > 0 && w.[0] >= 'A' && w.[0] <= 'Z' ->
      not_built st t "the refusal of a type written once in front of several parameters (D126, D132)";
      Error ()
  | _ ->
      let rec loop acc =
        match peek st with
        | Some ({ kind = Dollar name; _ } as t) -> (
            advance st;
            check_name st t ("$" ^ name) name;
            let* () =
              if next_is st (is_symbol ":") then begin
                advance st;
                Ok ()
              end
              else begin
                expected st "`:`";
                Error ()
              end
            in
            let* written = parse_type st in
            let parameter = { Core.parameter_name = name; parameter_at = place_of_token t; parameter_type = written } in
            match peek st with
            | Some c when is_symbol "," c ->
                advance st;
                loop (parameter :: acc)
            | Some c when is_symbol ")" c ->
                advance st;
                Ok (List.rev (parameter :: acc))
            | _ ->
                expected st "`)`";
                Error ())
        | _ ->
            expected st "a parameter, as in `$score: Whole`";
            Error ()
      in
      loop []

(* trace: D4, D8, D37, D38, D58, D132. `proc name(parameters) returns Type:`, its body, and `end proc name`. *)
let parse_definition st (keyword : token) : Core.definition option =
  begin_line st keyword 0;
  advance st;
  let header =
    match peek st with
    | Some dot when is_symbol "." dot -> (
        advance st;
        match peek st with
        | Some ({ kind = Word "main"; _ } as main) ->
            advance st;
            Some (Core.Main, main, "end proc.main")
        | _ ->
            expected st "`main`";
            None)
    | Some ({ kind = Word name; _ } as t) ->
        advance st;
        if is_keyword name then
          report st
            (M.message M.Refused (M.point t.line t.column t.width)
               ~what:(Printf.sprintf "`%s` is a keyword, so it can't be a procedure's name" name)
               ~why:"a procedure's name can't be one of Unio's own words.")
        else check_name st t name name;
        Some (Core.Named name, t, "end proc " ^ name)
    | _ ->
        expected st "a procedure's name";
        None
  in
  match header with
  | None ->
      st.stopped <- true;
      None
  | Some (name, name_token, closer) -> (
      let parameters =
        match peek st with
        | Some o when is_symbol "(" o -> (
            match name with
            | Core.Main ->
                let rec close () =
                  match peek st with
                  | Some c when is_symbol ")" c ->
                      advance st;
                      Some c
                  | Some c when c.line = o.line ->
                      advance st;
                      close ()
                  | _ -> None
                in
                (match close () with
                | Some c when c.column = o.column + 1 ->
                    report st
                      (M.message M.Refused (M.point o.line o.column 2)
                         ~fix:(M.replace_at (line_text st o.line) o.column 2 "")
                         ~why:"a definition writes `( )` only when it has parameters.")
                | Some c ->
                    report st
                      (M.message M.Refused
                         (M.point o.line o.column (c.column + 1 - o.column))
                         ~what:"`proc.main` takes nothing"
                         ~why:"only the program's start runs `proc.main`, so it's never given anything.")
                | None -> expected st "`)`");
                Ok []
            | Core.Named _ -> parse_parameters st o)
        | _ -> Ok []
      in
      match parameters with
      | Error () ->
          st.stopped <- true;
          None
      | Ok parameters -> (
          let returns =
            match peek st with
            | Some r when is_word "returns" r -> (
                advance st;
                match parse_type st with Ok t -> Ok (Some t) | Error () -> Error ())
            | _ -> Ok None
          in
          (match peek st with
          | Some f when is_word "fails" f || is_word "or" f ->
              not_built st f "failures listed in a header, `fails` (D39),";
              st.stopped <- true
          | _ -> ());
          match returns with
          | Error () ->
              st.stopped <- true;
              None
          | Ok returns ->
              if not st.stopped then expect_opener_colon st;
              if st.stopped then None
              else begin
                st.closers <- [ closer ];
                let body = parse_block st 1 in
                st.closers <- [];
                if not st.stopped then parse_closer st 0 closer;
                Some
                  { Core.definition_name = name; definition_name_at = place_of_token name_token;
                    definition_parameters = parameters; definition_returns = returns; definition_body = body;
                    definition_at = place_of_token keyword }
              end))

(* trace: D97, D101, D102. Two or more blank lines in a row. *)
let check_blank_lines st =
  let count = Array.length st.lines in
  let rec go i =
    if i < count then
      if String.trim st.lines.(i).text = "" && not st.lines.(i).comment then begin
        let rec run_end j = if j < count && String.trim st.lines.(j).text = "" then run_end (j + 1) else j in
        let stop = run_end i in
        let extra = stop - i - 1 in
        if extra >= 1 then
          report st
            (M.message M.Refused (M.point (i + 2) 1 1)
               ~what:
                 (Printf.sprintf "%d extra blank line%s, after the one on line %d" extra
                    (if extra = 1 then "" else "s") (i + 1))
               ~why:"one blank line is enough to separate the parts of a program.");
        go stop
      end
      else go (i + 1)
  in
  go 0

(* Reads a whole file: gives the definitions read, the problems found, and whether checking stopped early. *)
let parse (lines : Lexer.line list) =
  let lines = Array.of_list lines in
  let tokens = Array.of_list (List.concat_map (fun (l : Lexer.line) -> l.tokens) (Array.to_list lines)) in
  let st =
    { tokens; lines; index = 0; problems = []; stopped = false; statement_line = 0; spread_reported = false;
      closers = []; last_code_line = 0 }
  in
  check_blank_lines st;
  Array.iter
    (fun (l : Lexer.line) ->
      match l.doc_comment with
      | Some column ->
          report st
            (M.message M.Not_built_yet (M.point l.number column 3)
               ~what:"doc comments, `///` (D74), aren't built yet")
      | None -> ())
    lines;
  let rec definitions acc =
    if st.stopped then List.rev acc
    else
      match peek st with
      | None -> List.rev acc
      | Some t when is_word "proc" t -> (
          match parse_definition st t with
          | Some d -> definitions (d :: acc)
          | None -> List.rev acc)
      | Some t when is_word "extern" t ->
          not_built st t "procedures written in C++, `extern.cpp` (D36, D131),";
          st.stopped <- true;
          List.rev acc
      | Some t when is_word "fam" t ->
          not_built st t "families, `fam` (D41, D42),";
          st.stopped <- true;
          List.rev acc
      | Some t when is_word "comment" t ->
          not_built st t "block comments, `comment name:` (D74),";
          st.stopped <- true;
          List.rev acc
      | Some t ->
          begin_line st t 0;
          not_designed st t "code outside a procedure";
          st.stopped <- true;
          List.rev acc
  in
  let found = definitions [] in
  (* Comment lines after the last closer. *)
  (match peek st with
  | None when not st.stopped ->
      for number = st.last_code_line + 1 to Array.length lines do
        let l = lines.(number - 1) in
        if l.tokens = [] && l.comment && String.length l.text > 0 && (l.text.[0] = ' ' || l.text.[0] = '\t') then
          report st
            (M.message M.Not_designed_yet (M.point number 1 1)
               ~what:"how a comment after the last line of code is indented isn't decided yet")
      done
  | _ -> ());
  let stop_line = match peek st with Some t when st.stopped -> t.line | _ -> max_int in
  (found, List.rev st.problems, st.stopped, stop_line)
