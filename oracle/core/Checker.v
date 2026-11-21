(* The checker: what makes a program refused at compile time (D90), as data for the shell to word.

   It goes through every procedure, statement and expression, keeping track of the variables in use, and gives back
   every refusal it finds, in the order it meets them (D54, "every error is shown"). After an error it carries on
   (D55): a refused declaration still creates its variable with its written type, and a refused expression's type
   becomes unknown ([None] here), so nothing more is said about what uses it. *)

From Stdlib Require Import String List Bool ZArith.
From Ralph Require Import Syntax Whole Diagnostics Procedures.
Import ListNotations.
Open Scope string_scope.
Open Scope list_scope.

Inductive kind_of_variable := Let_variable | Var_variable | Parameter_variable.

(* What the checker knows about a variable in use. *)
Record variable_info := {
  variable_name : string;
  variable_type : type;
  variable_kind : kind_of_variable;
  variable_name_at : place;
  variable_keyword_at : place }.

(* Where the checker is. *)
Record setting := {
  setting_program : program;
  (* the variables in use, innermost first *)
  setting_scope : list variable_info;
  (* variables declared in blocks that have ended, for D93's `because:` *)
  setting_ended : list (string * place);
  setting_in_loop : bool;
  setting_procedure : string;
  setting_returns : option type }.

Definition with_scope (s : setting) (scope : list variable_info) : setting :=
  {| setting_program := setting_program s; setting_scope := scope; setting_ended := setting_ended s;
     setting_in_loop := setting_in_loop s; setting_procedure := setting_procedure s;
     setting_returns := setting_returns s |}.

Definition with_ended (s : setting) (ended : list (string * place)) : setting :=
  {| setting_program := setting_program s; setting_scope := setting_scope s; setting_ended := ended;
     setting_in_loop := setting_in_loop s; setting_procedure := setting_procedure s;
     setting_returns := setting_returns s |}.

Definition in_loop (s : setting) : setting :=
  {| setting_program := setting_program s; setting_scope := setting_scope s; setting_ended := setting_ended s;
     setting_in_loop := true; setting_procedure := setting_procedure s; setting_returns := setting_returns s |}.

Definition find_variable (name : string) (scope : list variable_info) : option variable_info :=
  find (fun info => String.eqb (variable_name info) name) scope.

Definition find_ended (name : string) (ended : list (string * place)) : option place :=
  match find (fun entry => String.eqb (fst entry) name) ended with
  | Some (_, at_) => Some at_
  | None => None
  end.

(* What a call gives back (D37, D38), and whether it can fail (D23). *)
Inductive gives := Gives_value (t : type) | Gives_nothing | Gives_unknown.

Record call_check := { call_gives : gives; call_can_fail : bool; call_diagnostics : list diagnostic }.

Definition unknown_call (diagnostics : list diagnostic) : call_check :=
  {| call_gives := Gives_unknown; call_can_fail := false; call_diagnostics := diagnostics |}.

(* How deep a chain of calls goes, each the first argument of the next, as `f(g(h($x)))` is 3 deep (D128). *)
Fixpoint chain_depth (e : expression) : nat :=
  match e with
  | Call _ (Argument _ first _) _ => S (chain_depth first)
  | Call _ No_arguments _ => 1
  | _ => 0
  end.

Definition parameter_labels (parameters : list (type * option string)) : list string :=
  map (fun p => match snd p with Some label => label | None => "" end) parameters.

Definition same_labels (a b : list string) : bool :=
  (length a =? length b)%nat && forallb (fun label => (count_occ string_dec a label =? count_occ string_dec b label)%nat) (a ++ b).

Definition is_maths (operator : binary_operator) : bool :=
  match operator with Add | Subtract | Multiply => true | _ => false end.

(* trace: D72. `&` right_operand next to unbracketed maths, on either side. *)
Definition beside_maths (operator : binary_operator) (left_operand right_operand : expression) (operator_at : place)
  : option diagnostic :=
  let maths_child e := match e with Binary o _ _ at_ _ => if is_maths o then Some at_ else None | _ => None end in
  let join_child e := match e with Binary Join _ _ at_ _ => Some at_ | _ => None end in
  match operator with
  | Join =>
      match maths_child left_operand, maths_child right_operand with
      | Some maths_at, _ | None, Some maths_at => Some (Refused (Join_beside_maths operator_at maths_at))
      | None, None => None
      end
  | Add | Subtract | Multiply =>
      match join_child left_operand, join_child right_operand with
      | Some join_at, _ | None, Some join_at => Some (Refused (Join_beside_maths join_at operator_at))
      | None, None => None
      end
  | _ => None
  end.

(* The type of an operator's result from its sides' types, or why it's refused. *)
Definition binary_type (operator : binary_operator) (a b : type) (left_operand right_operand : expression) (operator_at : place)
  : option type * list diagnostic :=
  match operator with
  | Add | Subtract | Multiply =>
      match a, b with
      | Whole, Whole => (Some Whole, [])
      | Text, Text =>
          match operator with
          | Add => (None, [Refused (Plus_on_text operator_at)])
          | _ => (None, [Refused (Arithmetic_types operator a b operator_at)])
          end
      | _, _ => (None, [Refused (Arithmetic_types operator a b operator_at)])
      end
  | Join =>
      match a, b with
      | Text, Text => (Some Text, [])
      | Bool, _ => (None, [Refused (Join_non_text Bool (expression_at left_operand))])
      | _, Bool => (None, [Refused (Join_non_text Bool (expression_at right_operand))])
      | _, _ =>
          let numbers :=
            (if type_eqb a Whole then [expression_at left_operand] else []) ++
            (if type_eqb b Whole then [expression_at right_operand] else []) in
          (None, [Refused (Join_number numbers)])
      end
  | Equal | Not_equal =>
      match a, b with
      | Whole, Whole | Bool, Bool => (Some Bool, [])
      | Text, Text => (None, [Not_designed_yet Comparing_text operator_at])
      | _, _ => (None, [Refused (Compare_types operator a b operator_at)])
      end
  | Less | Greater | At_least | At_most =>
      match a, b with
      | Whole, Whole => (Some Bool, [])
      | Text, Text => (None, [Not_designed_yet Comparing_text operator_at])
      | Bool, Bool => (None, [Not_designed_yet Ordering_bools operator_at])
      | _, _ => (None, [Refused (Compare_types operator a b operator_at)])
      end
  (* trace: D161 *)
  | And | Or =>
      match a, b with
      | Bool, Bool => (Some Bool, [])
      | Bool, other => (None, [Refused (Logic_on_non_bool operator other (expression_at right_operand))])
      | other, _ => (None, [Refused (Logic_on_non_bool operator other (expression_at left_operand))])
      end
  end.

(* An expression where a value of type [expected] goes, as in a declaration or an argument. *)
Definition expect (found : option type) (expected : type) (problem : type -> diagnostic) : list diagnostic :=
  match found with
  | Some t => if type_eqb t expected then [] else [problem t]
  | None => []
  end.

(* A call standing where a value goes (D38, D23). *)
Definition as_value (c : call_check) (at_ : place) : option type * list diagnostic :=
  match call_gives c, call_can_fail c with
  | Gives_value t, false => (Some t, call_diagnostics c)
  | Gives_value _, true => (None, call_diagnostics c ++ [Refused (Failure_unhandled at_)])
  | Gives_nothing, _ => (None, call_diagnostics c ++ [Refused (Gives_nothing_used at_)])
  | Gives_unknown, _ => (None, call_diagnostics c)
  end.

Definition is_call (e : expression) : bool := match e with Call _ _ _ => true | _ => false end.

Fixpoint count_steps (path : steps) : nat :=
  match path with
  | No_steps => 0
  | Step _ _ _ _ rest => S (count_steps rest)
  end.

(* trace: D128. A slide is for a chain of three calls or more, written as a slide all the way. *)
Definition slide_shape (start : expression) (path : steps) (at_ : place) : list diagnostic :=
  let depth := chain_depth start in
  if (2 <=? depth)%nat then [Refused (Calls_as_slide (Slide start path at_))]
  else if (depth + count_steps path <? 3)%nat then [Refused (Slide_as_calls (Slide start path at_))]
  else [].

(* Checks one piece of `.fmt` text (D29, D133). *)
Definition check_piece (s : setting) (p : piece) : list diagnostic :=
  match p with
  | Piece_text _ => []
  | Piece_variable name at_ =>
      match find_variable name (setting_scope s) with
      | Some info =>
          match variable_type info with
          | Whole | Text => []
          | Bool => [Not_designed_yet Format_bool at_]
          end
      | None => [Refused (Brace_not_variable name at_)]
      end
  | Piece_bare_name name at_ =>
      match find_variable name (setting_scope s) with
      | Some _ => [Refused (Brace_without_dollar at_)]
      | None => [Refused (Brace_not_variable name at_)]
      end
  | Piece_other at_ => [Refused (Brace_calculation at_)]
  | Piece_unclosed at_ => [Not_designed_yet Format_unclosed at_]
  end.

(* An argument, already checked by its own lights. *)
Record checked_argument := {
  argument_label : option (string * place);
  argument_value : expression;
  argument_type : option type;
  argument_diagnostics : list diagnostic }.

Definition argument_at (a : checked_argument) : place := expression_at (argument_value a).

Definition values_diagnostics (given : list checked_argument) : list diagnostic :=
  flat_map argument_diagnostics given.

Definition argument_labels (given : list checked_argument) : list string :=
  map (fun a => match argument_label a with Some (label, _) => label | None => "" end) given.

(* Labels written in another order than the procedure's parameters, which isn't decided (D24, D25). *)
Definition labels_reordered (parameters : list (type * option string)) (given : list checked_argument) : bool :=
  let written := argument_labels given in
  let expected := parameter_labels parameters in
  (if list_eq_dec string_dec written expected then false else true) && same_labels written expected.

(* Checks arguments against parameters, each a type and a label, the counts already known to match (D24). *)
Fixpoint check_pairs (procedure : string) (parameters : list (type * option string)) (given : list checked_argument)
  : list diagnostic :=
  match given, parameters with
  | a :: rest, (expected, wanted) :: others =>
      let label_problems :=
        match wanted, argument_label a with
        | Some w, None => [Refused (Missing_label w (argument_at a))]
        | None, Some (_, label_at) => [Refused (Label_where_none label_at (argument_at a))]
        | Some w, Some (l, label_at) => if String.eqb w l then [] else [Refused (Wrong_label w l label_at)]
        | None, None => []
        end in
      argument_diagnostics a ++ label_problems
        ++ expect (argument_type a) expected
             (fun found => Refused (Argument_type procedure expected found (argument_at a)))
        ++ check_pairs procedure others rest
  | _, _ => []
  end.

(* Checks a call against a procedure's parameters, counting first (D24, D88). *)
Definition check_against (procedure : string) (parameters : list (type * option string))
    (given : list checked_argument) (at_ : place) (declared_at : option place) : list diagnostic :=
  if negb (Nat.eqb (length given) (length parameters)) then
    values_diagnostics given
      ++ [Refused (Argument_count procedure (length parameters) (length given) at_ declared_at)]
  else if labels_reordered parameters given then values_diagnostics given ++ [Not_designed_yet Label_order at_]
  else check_pairs procedure parameters given.

(* `be.Text` and `be.Whole` take one value without a label (D22): gives its type and place. *)
Definition check_single (procedure : string) (given : list checked_argument) (at_ : place)
  : option (type * place) * list diagnostic :=
  match given with
  | [a] =>
      match argument_label a with
      | None =>
          match argument_type a with
          | Some t => (Some (t, argument_at a), argument_diagnostics a)
          | None => (None, argument_diagnostics a)
          end
      | Some (_, label_at) =>
          (None, argument_diagnostics a ++ [Refused (Label_where_none label_at (argument_at a))])
      end
  | _ => (None, values_diagnostics given ++ [Refused (Argument_count procedure 1 (length given) at_ None)])
  end.

(* `.fmt`'s one piece of text, written in quotes (D29). *)
Definition check_format (s : setting) (given : list checked_argument) (at_ : place) : list diagnostic :=
  match given with
  | [a] =>
      match argument_label a, argument_value a with
      | Some (_, label_at), _ => argument_diagnostics a ++ [Refused (Format_text_label label_at)]
      | None, Text_literal _ pieces _ => argument_diagnostics a ++ flat_map (check_piece s) pieces
      | None, value => argument_diagnostics a ++ [Not_designed_yet Format_not_literal (expression_at value)]
      end
  | _ => values_diagnostics given ++ [Refused (Format_without_text at_)]
  end.

(* A whole number written plainly in the code, as `6` or `-1`. *)
Definition plain_number (e : expression) : option Z :=
  match e with
  | Whole_literal digits _ => whole_of_literal digits
  | Negate (Whole_literal digits _) _ => option_map Z.opp (whole_of_literal digits)
  | _ => None
  end.

(* trace: D160. `random` given two plain numbers with `from:` above `to:` is refused, with them swapped as the fix. *)
Definition random_backwards (given : list checked_argument) (at_ : place) : list diagnostic :=
  match given with
  | [low; high] =>
      match plain_number (argument_value low), plain_number (argument_value high) with
      | Some l, Some h =>
          if (h <? l)%Z then [Refused (Random_backwards_plain at_ (argument_at low) (argument_at high))] else []
      | _, _ => []
      end
  | _ => []
  end.

Definition gives_of (d : definition) : gives :=
  match definition_returns d with Some t => Gives_value t | None => Gives_nothing end.

Definition make_call (gives : gives) (can_fail : bool) (diagnostics : list diagnostic) : call_check :=
  {| call_gives := gives; call_can_fail := can_fail; call_diagnostics := diagnostics |}.

(* A call of [target], once its arguments are checked. *)
Definition check_target (s : setting) (procedure : string) (target : target) (given : list checked_argument)
    (at_ : place) : call_check :=
  match target with
  | Library (Show_message false _) => make_call Gives_nothing false (check_against procedure [(Text, None)] given at_ None)
  | Library (Show_message true _) => make_call Gives_nothing false (check_format s given at_)
  | Library (Convert_to_text true) => make_call (Gives_value Text) false (check_format s given at_)
  | Library (Convert_to_text false) =>
      match check_single procedure given at_ with
      (* trace: D22, D153 *)
      | (Some (Whole, _), d) | (Some (Bool, _), d) => make_call (Gives_value Text) false d
      | (Some (Text, value_at), d) => unknown_call (d ++ [Refused (Convert_to_same Text value_at)])
      | (None, d) => unknown_call d
      end
  | Library Convert_to_whole =>
      match check_single procedure given at_ with
      | (Some (Text, _), d) => make_call (Gives_value Whole) true d
      | (Some (Whole, value_at), d) => unknown_call (d ++ [Refused (Convert_to_same Whole value_at)])
      | (Some (Bool, value_at), d) => unknown_call (d ++ [Not_designed_yet Converting value_at])
      | (None, d) => unknown_call d
      end
  | Library Ask_for_line => make_call (Gives_value Text) false (check_against procedure [(Text, None)] given at_ None)
  | Library Random_number =>
      make_call (Gives_value Whole) false
        (check_against procedure [(Whole, Some "from"); (Whole, Some "to")] given at_ None
           ++ random_backwards given at_)
  | User d =>
      make_call (gives_of d) false (check_against procedure (user_parameters d) given at_ (Some (definition_name_at d)))
  end.

(* trace: D4, D24, D38, D128 *)
Definition check_call (s : setting) (name : callee) (given : list checked_argument) (at_ : place) : call_check :=
  match resolve (setting_program s) name with
  | Unresolved problems => unknown_call (problems ++ values_diagnostics given)
  | Resolved target => check_target s (callee_name name) target given at_
  end.

(* trace: D128, D142. Three or more calls nested, each the first argument of the next, are written as a slide. The
   shell keeps only the outermost of a chain's refusals (D55). *)
Definition chain_problem (e : expression) : list diagnostic :=
  if (3 <=? chain_depth e)%nat then [Refused (Calls_as_slide e)] else [].

(* A slide's step: [target] called with the slid value first, then the step's own arguments (D5). *)
Definition check_step_target (s : setting) (procedure : string) (target : target) (slid : option type)
    (given : list checked_argument) (at_ : place) : call_check :=
  let one_more (parameters : list (type * option string)) (gives : gives) (can_fail : bool) :=
    match parameters with
    | [] => unknown_call (values_diagnostics given ++ [Refused (Argument_count procedure 0 (S (length given)) at_ None)])
    | (first, _) :: others =>
        make_call gives can_fail
          (expect slid first (fun found => Refused (Slid_value_type procedure first found at_))
             ++ check_against procedure others given at_ None)
    end in
  match target with
  | Library (Show_message false _) => one_more [(Text, None)] Gives_nothing false
  | Library (Show_message true _) | Library (Convert_to_text true) =>
      unknown_call (values_diagnostics given ++ [Not_designed_yet Format_not_literal at_])
  | Library (Convert_to_text false) =>
      match slid with
      | Some Text => unknown_call (values_diagnostics given ++ [Refused (Convert_to_same Text at_)])
      | Some Bool => one_more [(Bool, None)] (Gives_value Text) false
      | _ => one_more [(Whole, None)] (Gives_value Text) false
      end
  | Library Convert_to_whole =>
      match slid with
      | Some Whole => unknown_call (values_diagnostics given ++ [Refused (Convert_to_same Whole at_)])
      | Some Bool => unknown_call (values_diagnostics given ++ [Not_designed_yet Converting at_])
      | _ => one_more [(Text, None)] (Gives_value Whole) true
      end
  | Library Ask_for_line => one_more [(Text, None)] (Gives_value Text) false
  | Library Random_number => one_more [(Whole, Some "from"); (Whole, Some "to")] (Gives_value Whole) false
  | User d => one_more (user_parameters d) (gives_of d) false
  end.

Definition check_step (s : setting) (name : callee) (slid : option type) (given : list checked_argument) (at_ : place)
  : call_check :=
  match resolve (setting_program s) name with
  | Unresolved problems => unknown_call (problems ++ values_diagnostics given)
  | Resolved target => check_step_target s (callee_name name) target slid given at_
  end.

Fixpoint check_expression (s : setting) (e : expression) {struct e} : option type * list diagnostic :=
  match e with
  | Whole_literal digits at_ =>
      match whole_of_literal digits with
      | Some n =>
          if (whole_minimum <=? n)%Z && (n <=? whole_maximum)%Z then (Some Whole, [])
          else (None, [Not_designed_yet Whole_literal_out_of_range at_])
      | None => (None, [Already_refused at_])
      end
  | Text_literal _ _ _ => (Some Text, [])
  | Bool_literal _ _ => (Some Bool, [])
  | Variable_value name at_ =>
      match find_variable name (setting_scope s) with
      | Some info => (Some (variable_type info), [])
      | None => (None, [Refused (Unknown_variable name at_ (find_ended name (setting_ended s)))])
      end
  | Bare_word word at_ =>
      if is_library_word word then (None, [Refused (Procedure_without_brackets word at_)])
      else
        match find_definition (setting_program s) word with
        | Some _ => (None, [Refused (Procedure_without_brackets word at_)])
        | None => (None, [Not_designed_yet Bare_word_not_procedure at_])
        end
  | Negate operand at_ =>
      let (t, ds) := check_expression s operand in
      match t with
      | Some Whole => (Some Whole, ds)
      | Some other => (None, ds ++ [Refused (Negate_non_number other at_)])
      | None => (None, ds)
      end
  | Not operand at_ =>
      let (t, ds) := check_expression s operand in
      match t with
      | Some Bool => (Some Bool, ds)
      (* trace: D161 *)
      | Some other => (None, ds ++ [Refused (Not_on_non_bool other (expression_at operand))])
      | None => (None, ds)
      end
  | Binary operator left_operand right_operand operator_at _ =>
      let (a, dl) := check_expression s left_operand in
      let (b, dr) := check_expression s right_operand in
      match beside_maths operator left_operand right_operand operator_at with
      | Some problem => (None, dl ++ dr ++ [problem])
      | None =>
          match a, b with
          | Some a', Some b' =>
              let (t, d) := binary_type operator a' b' left_operand right_operand operator_at in (t, dl ++ dr ++ d)
          | _, _ => (None, dl ++ dr)
          end
      end
  | Bracketed inner _ => check_expression s inner
  | Call name given at_ =>
      let (t, d) := as_value (check_call s name (check_arguments s given) at_) at_ in
      (t, d ++ chain_problem (Call name given at_))
  | Fallback attempt fallback operator_at _ =>
      let (f, df) := check_expression s fallback in
      match attempt with
      | Call name given call_at =>
          let c := check_call s name (check_arguments s given) call_at in
          let chain := chain_problem (Call name given call_at) in
          match call_gives c, call_can_fail c with
          | Gives_value t, true =>
              (Some t, call_diagnostics c ++ chain ++ df
                         ++ expect f t (fun found => Refused (Fallback_type t found (expression_at fallback))))
          | Gives_unknown, _ => (None, call_diagnostics c ++ chain ++ df)
          | _, _ => (None, call_diagnostics c ++ chain ++ df ++ [Refused (Fallback_without_failing_call operator_at)])
          end
      | _ =>
          let (_, da) := check_expression s attempt in
          (None, da ++ df ++ [Refused (Fallback_without_failing_call operator_at)])
      end
  | Slide start path at_ =>
      let (t, ds) := check_expression s start in
      let (result, dc) := as_value (check_steps s t path) at_ in
      (result, ds ++ slide_shape start path at_ ++ dc)
  | Broken at_ => (None, [Already_refused at_])
  end

(* Checks each argument by its own lights. *)
with check_arguments (s : setting) (given : arguments) {struct given} : list checked_argument :=
  match given with
  | No_arguments => []
  | Argument label value rest =>
      let (t, d) := check_expression s value in
      {| argument_label := label; argument_value := value; argument_type := t; argument_diagnostics := d |}
        :: check_arguments s rest
  end

(* A slide's steps, each given the value slid into it as its first argument (D5); gives the last step's call. *)
with check_steps (s : setting) (slid : option type) (path : steps) {struct path} : call_check :=
  match path with
  | No_steps => unknown_call []
  | Step name _ given at_ rest =>
      let this := check_step s name slid (check_arguments s given) at_ in
      match rest with
      | No_steps => this
      | Step _ _ _ _ _ =>
          let (next, d) := as_value this at_ in
          let later := check_steps s next rest in
          make_call (call_gives later) (call_can_fail later) (d ++ call_diagnostics later)
      end
  end.

(* A call or a slide standing alone as a statement (D120). *)
Definition check_run_call (c : call_check) (procedure : string) (at_ : place) : list diagnostic :=
  match call_gives c, call_can_fail c with
  | _, true => call_diagnostics c ++ [Refused (Failure_unhandled at_)]
  | Gives_value t, false => call_diagnostics c ++ [Refused (Value_unused procedure t at_)]
  | _, false => call_diagnostics c
  end.

Fixpoint last_step_name (path : steps) : string :=
  match path with
  | No_steps => ""
  | Step name _ _ _ No_steps => callee_name name
  | Step _ _ _ _ rest => last_step_name rest
  end.

Definition check_run (s : setting) (action : expression) : list diagnostic :=
  match action with
  | Call name given at_ =>
      check_run_call (check_call s name (check_arguments s given) at_) (callee_name name) at_
        ++ chain_problem action
  | Slide start path at_ =>
      let (t, ds) := check_expression s start in
      ds ++ slide_shape start path at_ ++ check_run_call (check_steps s t path) (last_step_name path) at_
  | Fallback (Call name _ _) _ _ at_ =>
      let (t, ds) := check_expression s action in
      match t with
      | Some t' => ds ++ [Refused (Value_unused (callee_name name) t' at_)]
      | None => ds
      end
  | Broken at_ => [Already_refused at_]
  (* trace: D161. A line that only works out a value; a refused one has had its say already (D55). *)
  | _ =>
      let (t, ds) := check_expression s action in
      ds ++ [match t with
             | Some t' => Refused (Value_only_line (expression_at action) t')
             | None => Already_refused (expression_at action)
             end]
  end.

(* A condition, for `if` and `while` (D33). *)
Definition check_condition (s : setting) (condition : expression) : list diagnostic :=
  let (t, ds) := check_expression s condition in
  match t with
  | Some Bool | None => ds
  | Some _ => ds ++ [Not_designed_yet Condition_not_bool (expression_at condition)]
  end.

(* Whether a statement leaves the block it's in: by `return`, or by `break` when it isn't inside a loop of its own
   ([in_inner_loop]). *)
Fixpoint can_leave (in_inner_loop : bool) (st : statement) : bool :=
  match st with
  | Return _ _ => true
  | Break _ => negb in_inner_loop
  | If _ then_part rest _ => can_leave_block in_inner_loop then_part || can_leave_else in_inner_loop rest
  | While _ body _ => can_leave_block true body
  | _ => false
  end
with can_leave_block (in_inner_loop : bool) (b : block) : bool :=
  match b with
  | Empty => false
  | Then st rest => can_leave in_inner_loop st || can_leave_block in_inner_loop rest
  end
with can_leave_else (in_inner_loop : bool) (e : else_part) : bool :=
  match e with
  | No_else => false
  | Else body _ => can_leave_block in_inner_loop body
  | Else_if _ body rest _ => can_leave_block in_inner_loop body || can_leave_else in_inner_loop rest
  end.

(* trace: D9, D57, D117. What's refused about [next] coming after [st] in the same block. *)
Definition follows (st next : statement) : list diagnostic :=
  match st with
  | Return _ at_ => [Refused (After_leaving true (statement_at next) at_)]
  | Break at_ => [Refused (After_leaving false (statement_at next) at_)]
  | If _ _ _ at_ => if can_leave false st then [Refused (After_if_that_leaves (statement_at next) at_)] else []
  | _ => []
  end.

(* Whether every way through ends in `return` with a value (D57). *)
Fixpoint always_returns (st : statement) : bool :=
  match st with
  | Return (Some _) _ => true
  | If _ then_part rest _ => always_returns_block then_part && always_returns_else rest
  | _ => false
  end
with always_returns_block (b : block) : bool :=
  match b with
  | Empty => false
  | Then st Empty => always_returns st
  | Then _ rest => always_returns_block rest
  end
with always_returns_else (e : else_part) : bool :=
  match e with
  | No_else => false
  | Else body _ => always_returns_block body
  | Else_if _ body rest _ => always_returns_block body && always_returns_else rest
  end.

Definition declare (s : setting) (info : variable_info) : setting := with_scope s (info :: setting_scope s).

(* The names a block declared, for D93's `because:` once it's ended. *)
Definition declared_in (st : statement) : list (string * place) :=
  match st with
  | Declare _ name name_at _ _ _ _ => [(name, name_at)]
  | _ => []
  end.

Fixpoint check_statement (s : setting) (st : statement) {struct st} : setting * list diagnostic :=
  match st with
  | Declare changeable name name_at keyword_at written value _ =>
      let (t, ds) := check_expression s value in
      let in_use :=
        match find_variable name (setting_scope s) with
        | Some info => [Refused (Name_in_use name name_at (variable_name_at info))]
        | None => []
        end in
      let mismatch := expect t written (fun found => Refused (Wrong_value_type name written found (expression_at value))) in
      let info := {| variable_name := name; variable_type := written;
                     variable_kind := if changeable then Var_variable else Let_variable;
                     variable_name_at := name_at; variable_keyword_at := keyword_at |} in
      (declare s info, in_use ++ ds ++ mismatch)
  | Assign name name_at value _ _ =>
      let (t, ds) := check_expression s value in
      match find_variable name (setting_scope s) with
      | None => (s, [Refused (Unknown_variable name name_at (find_ended name (setting_ended s)))] ++ ds)
      | Some info =>
          match variable_kind info with
          | Let_variable => (s, [Refused (Let_cannot_change name name_at (variable_keyword_at info))] ++ ds)
          | Parameter_variable =>
              (s, [Refused (Parameter_cannot_change name name_at (variable_name_at info))] ++ ds)
          | Var_variable =>
              (s, ds ++ expect t (variable_type info)
                          (fun found => Refused (Wrong_value_type name (variable_type info) found (expression_at value))))
          end
      end
  | Run action _ => (s, check_run s action)
  | If condition then_part rest _ =>
      let dc := check_condition s condition in
      let (ended_then, dt) := check_block s then_part in
      let (ended_else, de) := check_else s rest in
      (with_ended s (ended_then ++ ended_else ++ setting_ended s), dc ++ dt ++ de)
  | While condition body _ =>
      let dc := check_condition s condition in
      let (ended_body, db) := check_block (in_loop s) body in
      (with_ended s (ended_body ++ setting_ended s), dc ++ db)
  | Break at_ => (s, if setting_in_loop s then [] else [Refused (Break_outside_loop at_)])
  | Return None at_ =>
      (s, match setting_returns s with
          | Some t => [Refused (Bare_return_with_returns (setting_procedure s) t at_)]
          | None => []
          end)
  | Return (Some value) at_ =>
      let (t, ds) := check_expression s value in
      (s, match setting_returns s with
          | Some expected => ds ++ expect t expected (fun _ => Not_designed_yet Return_type_wording (expression_at value))
          | None => ds ++ [Refused (Return_value_without_returns (setting_procedure s) at_)]
          end)
  | Labels _ at_ => (s, [Refused (Labels_not_first at_)])
  end

(* Checks a block's statements in turn; gives back the names it declared, which end with it (D93). *)
with check_block (s : setting) (b : block) {struct b} : list (string * place) * list diagnostic :=
  match b with
  | Empty => ([], [])
  | Then st rest =>
      let (s', ds) := check_statement s st in
      let after := match rest with Then next _ => follows st next | Empty => [] end in
      let (names, dr) := check_block s' rest in
      (declared_in st ++ names, ds ++ after ++ dr)
  end

with check_else (s : setting) (e : else_part) {struct e} : list (string * place) * list diagnostic :=
  match e with
  | No_else => ([], [])
  | Else body _ => check_block s body
  | Else_if condition body rest _ =>
      let dc := check_condition s condition in
      let (ended_body, db) := check_block s body in
      let (ended_rest, dr) := check_else s rest in
      (ended_body ++ ended_rest, dc ++ db ++ dr)
  end.

Fixpoint last_statement (b : block) : option statement :=
  match b with
  | Empty => None
  | Then st Empty => Some st
  | Then _ rest => last_statement rest
  end.

(* trace: D102, D126. The second of two parameters with one name. *)
Fixpoint repeated_parameters (seen : list string) (parameters : list parameter) : list diagnostic :=
  match parameters with
  | [] => []
  | p :: rest =>
      (if existsb (String.eqb (parameter_name p)) seen
       then [Refused (Two_parameters_one_name (parameter_name p) (parameter_at p))] else [])
        ++ repeated_parameters (parameter_name p :: seen) rest
  end.

(* trace: D25, D89. Checks a `labels` line. *)
Fixpoint check_label_pairs (procedure : string) (parameters : list parameter) (seen_parameters seen_labels : list string)
    (pairs : list label_pair) : list diagnostic :=
  match pairs with
  | [] => []
  | one_pair :: rest =>
      let missing :=
        if existsb (fun p => String.eqb (parameter_name p) (pair_parameter one_pair)) parameters then []
        else [Refused (Label_for_missing_parameter procedure (pair_parameter one_pair) (pair_parameter_at one_pair))] in
      let twice :=
        if existsb (String.eqb (pair_parameter one_pair)) seen_parameters
        then [Refused (Two_labels_one_parameter (pair_parameter one_pair) (pair_parameter_at one_pair))] else [] in
      let shared :=
        if existsb (String.eqb (pair_label one_pair)) seen_labels
        then [Refused (One_label_two_parameters (pair_label one_pair) (pair_label_at one_pair))] else [] in
      missing ++ twice ++ shared
        ++ check_label_pairs procedure parameters (pair_parameter one_pair :: seen_parameters)
             (pair_label one_pair :: seen_labels) rest
  end.

Definition parameter_info (p : parameter) : variable_info :=
  {| variable_name := parameter_name p; variable_type := parameter_type p; variable_kind := Parameter_variable;
     variable_name_at := parameter_at p; variable_keyword_at := parameter_at p |}.


(* A body's `labels` line, if it starts with one (D25), and the statements after it. *)
Definition labels_and_rest (b : block) : option (list label_pair) * block :=
  match b with
  | Then (Labels pairs _) rest => (Some pairs, rest)
  | _ => (None, b)
  end.

(* trace: D4, D25, D37, D57, D88 *)
Definition check_definition (p : program) (d : definition) : list diagnostic :=
  let procedure := procedure_label d in
  let start := {| setting_program := p; setting_scope := rev (map parameter_info (definition_parameters d));
                  setting_ended := []; setting_in_loop := false; setting_procedure := procedure;
                  setting_returns := definition_returns d |} in
  let (labels_line, rest) := labels_and_rest (definition_body d) in
  let labels_problems :=
    match labels_line with
    | Some pairs => check_label_pairs procedure (definition_parameters d) [] [] pairs
    | None => []
    end in
  let main_returns :=
    match definition_name d, definition_returns d with
    | Main, Some _ => [Not_designed_yet Main_returns (definition_name_at d)]
    | _, _ => []
    end in
  (* `proc.main` takes nothing (D4, D88): the parser has already refused any parameters. *)
  let main_parameters :=
    match definition_name d, definition_parameters d with
    | Main, _ :: _ => [Already_refused (definition_name_at d)]
    | _, _ => []
    end in
  let last_line :=
    match last_statement rest with
    | Some (Return None at_) => [Refused (Bare_return_last at_)]
    | _ => []
    end in
  let missing_return :=
    match definition_returns d with
    | Some t => if always_returns_block rest then [] else [Refused (Missing_return procedure t (definition_name_at d))]
    | None => []
    end in
  repeated_parameters [] (definition_parameters d) ++ main_returns ++ main_parameters ++ labels_problems
    ++ snd (check_block start rest) ++ last_line ++ missing_return.

(* trace: D4, D64, D92. Exactly one `proc.main`, and one procedure to a name. *)
Fixpoint check_names (mains : list place) (names : list (string * place)) (p : program) : list diagnostic :=
  match p with
  | [] => match mains with [] => [Refused No_main] | _ => [] end
  | d :: rest =>
      match definition_name d with
      | Main =>
          match mains with
          | first :: _ => [Refused (Two_mains (definition_name_at d) first)]
          | [] => []
          end ++ check_names (mains ++ [definition_name_at d]) names rest
      | Named n =>
          (if is_library_word n then [Not_designed_yet Library_name_taken (definition_name_at d)] else [])
            ++ match find (fun entry => String.eqb (fst entry) n) names with
               | Some (_, first) => [Refused (Two_procedures_one_name n (definition_name_at d) first)]
               | None => []
               end
            ++ check_names mains (names ++ [(n, definition_name_at d)]) rest
      end
  end.

(* THE CHECKER: every refusal in a program, and every place Ralph must stop because something isn't decided.
   A program is accepted when this is empty. *)
Definition check_program (p : program) : list diagnostic :=
  check_names [] [] p ++ flat_map (check_definition p) p.
