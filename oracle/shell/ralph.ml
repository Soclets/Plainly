(* Ralph, the oracle (D129, D141): `ralph program.unio` checks a Unio program and runs it.

   This is the shell around the proven core: it reads and decodes the file, parses it, has the core check it, words
   the messages (D54), and runs the core's evaluator, doing the real printing, reading and random numbers it asks for.
   Exit statuses are D56's and D82's. *)

module M = Messages

(* Far more steps than any program will take; a program still running after them stops as an internal error. *)
let fuel = max_int

let line_open = ref false

(* trace: D104. A message never starts mid-line: an open output line is ended on standard output first. *)
let end_open_line () =
  if !line_open then begin
    print_char '\n';
    line_open := false
  end;
  flush stdout

let finish status =
  flush stdout;
  flush stderr;
  exit status

let print_messages file source messages =
  List.iter (fun m -> prerr_string (M.render file source m)) messages

(* A whole number from [low] to [high], each equally likely (D26): random bits, tried again until they fall in
   range. The numbers stay Rocq's exact integers throughout. *)
let random_between low high =
  let width = Core.Z.sub high low in
  let bits = match width with Core.Zpos p -> Core.Coq_Pos.size_nat p | Core.Z0 | Core.Zneg _ -> 0 in
  let rec draw () =
    let rec build n so_far =
      if n = 0 then so_far
      else build (n - 1) (if Random.bool () then Core.Z.succ_double so_far else Core.Z.double so_far)
    in
    let candidate = build bits Core.Z0 in
    if Core.Z.leb candidate width then Core.Z.add low candidate else draw ()
  in
  draw ()

(* trace: D20, D96, D144. One line of input, without its line break; `\r\n` is one line break. A last line with no
   line break after it still counts; no input at all gives [None]. *)
let read_answer () =
  let buffer = Buffer.create 80 in
  let rec go () =
    match In_channel.input_char stdin with
    | Some '\n' ->
        let line = Buffer.contents buffer in
        Some
          (if String.ends_with ~suffix:"\r" line then String.sub line 0 (String.length line - 1) else line)
    | Some c ->
        Buffer.add_char buffer c;
        go ()
    | None -> if Buffer.length buffer = 0 then None else Some (Buffer.contents buffer)
  in
  go ()

let arithmetic_symbol = function Core.Plus -> "+" | Core.Minus -> "-" | Core.Times -> "*"

(* Walks the evaluator's tree of requests (core/Io.v), doing each one. *)
let rec walk file source (tree : unit Core.io) =
  match tree with
  | Core.Done () -> finish 0
  | Core.Show (text, ends_line, next) ->
      print_string text;
      if ends_line then begin
        print_char '\n';
        line_open := false
      end
      else if text <> "" then line_open := true;
      flush stdout;
      walk file source (next ())
  | Core.Ask (question, next) ->
      print_string question;
      flush stdout;
      if question <> "" then line_open := true;
      let answer = read_answer () in
      (* trace: D106. Typed at a terminal, the player's Enter has ended the line. *)
      if Unix.isatty Unix.stdin then line_open := false;
      walk file source (next answer)
  | Core.Random (low, high, next) -> walk file source (next (random_between low high))
  | Core.Stop reason -> (
      match reason with
      (* trace: D96 *)
      | Core.Input_ended -> finish 0
      (* trace: D46, D92 *)
      | Core.Overflow (p, too_big, Core.Operation_binary (a, operator, b)) ->
          end_open_line ();
          print_messages file source
            [ M.message M.Check_failed (Wording.at p)
                ~what:(if too_big then "the result is too big for a `Whole`" else "the result is too small for a `Whole`")
                ~when_:
                  (Printf.sprintf "%s %s %s" (Core.text_of_whole a) (arithmetic_symbol operator) (Core.text_of_whole b))
                ~why:"a `Whole` holds whole numbers from -9223372036854775808 to 9223372036854775807." ];
          finish 4
      | Core.Overflow (p, too_big, Core.Operation_negate a) ->
          end_open_line ();
          let value = Core.text_of_whole a in
          (* trace: D142. The value is in brackets when it starts with `-`. *)
          let shown = if String.length value > 0 && value.[0] = '-' then "-(" ^ value ^ ")" else "-" ^ value in
          print_messages file source
            [ M.message M.Check_failed (Wording.at p)
                ~what:(if too_big then "the result is too big for a `Whole`" else "the result is too small for a `Whole`")
                ~when_:shown
                ~why:"a `Whole` holds whole numbers from -9223372036854775808 to 9223372036854775807." ];
          finish 4
      | Core.Random_range_not_designed (p, _, _) ->
          end_open_line ();
          print_messages file source
            [ M.message M.Not_designed_yet (Wording.at p) ~what:"`random` with `from:` above `to:` isn't decided yet" ];
          finish 3
      | Core.Out_of_fuel ->
          end_open_line ();
          prerr_string (M.internal_error "the program ran more steps than Ralph can count");
          finish 7
      | Core.Stuck ->
          end_open_line ();
          prerr_string (M.internal_error "the evaluator met something it has no rule for");
          finish 7)

let position (m : M.message) = (m.at.line, M.first_column m.at)

(* trace: D55, D128. One chain of calls is one mistake: of the refusals for a chain and the chains inside it, only
   the outermost is kept. *)
let outermost_chains messages =
  let chain (m : M.message) = m.why = Some Wording.chain_why in
  let span (m : M.message) =
    match m.at.carets with (column, width) :: _ -> (m.at.line, column, column + width) | [] -> (m.at.line, 0, 0)
  in
  let inside a b =
    let line_a, first_a, stop_a = span a and line_b, first_b, stop_b = span b in
    line_a = line_b && first_b <= first_a && stop_a <= stop_b && (first_a, stop_a) <> (first_b, stop_b)
  in
  List.filter (fun m -> not (chain m && List.exists (fun other -> chain other && inside m other) messages)) messages

(* trace: D56, D92, D103 *)
let read file =
  if not (Filename.check_suffix file ".unio") then Error "a Unio file's name ends in `.unio`"
  else
    match In_channel.with_open_bin file In_channel.input_all with
    | exception Sys_error message ->
        let prefix = file ^ ": " in
        let reason =
          if String.starts_with ~prefix message then
            String.sub message (String.length prefix) (String.length message - String.length prefix)
          else message
        in
        Error reason
    | contents -> if String.is_valid_utf_8 contents then Ok contents else Error "it isn't UTF-8 text"

let () =
  Random.self_init ();
  match Sys.argv with
  | [| _; file |] -> (
      match read file with
      | Error reason ->
          prerr_string (M.cant_read file reason);
          finish 2
      | Ok contents ->
          let lines = Lexer.lines_of contents in
          let source = Array.of_list (List.map (fun (l : Lexer.line) -> l.text) lines) in
          let carriage_return =
            List.find_map
              (fun (l : Lexer.line) ->
                Option.map
                  (fun i ->
                    M.message M.Not_designed_yet (M.point l.number (i + 1) 1)
                      ~what:"a `\\r` that isn't part of a line break isn't decided yet")
                  (String.index_opt l.text '\r'))
              lines
          in
          let definitions, parse_problems, stopped, stop_line = Parser.parse lines in
          let core_problems =
            List.filter_map (Wording.diagnostic source) (Core.check_program definitions)
            |> List.filter (fun (m : M.message) ->
                   (not stopped) || (m.at.line < stop_line && m.what <> Some "this program has no `proc.main`"))
          in
          let messages =
            List.stable_sort
              (fun a b -> compare (position a) (position b))
              (outermost_chains
                 (Option.to_list carriage_return @ parse_problems @ Spacing.check lines @ core_problems))
          in
          if messages <> [] then begin
            print_messages file source messages;
            (* trace: D56, D148. Any refusal gives status 1; only limits alone give 3. *)
            let refused = List.exists (fun (m : M.message) -> m.kind = M.Refused) messages in
            finish (if refused then 1 else 3)
          end
          else walk file source (Core.run fuel definitions))
  | _ ->
      prerr_string "ralph: give it one Unio file, as in `ralph game.unio`\n";
      finish 6
