(* The evaluator: what a Unio program does when it runs.

   It works on the same syntax tree the checker reads, and gives back a tree of requests (Io.v) for the shell to carry
   out. Rocq only accepts functions that finish, so every step uses up one unit of fuel, a counter the shell sets far
   higher than any real program needs; a program still running when it's used up stops with [Out_of_fuel].

   Whenever the evaluator meets something it has no rule for, like adding a `Bool`, it stops with [Stuck]. The checker
   refuses every such program before it runs: that's the type-safety theorem in Safety.v. *)

From Stdlib Require Import String List Bool ZArith.
From Ralph Require Import Syntax Whole Io Procedures.
Import ListNotations.
Open Scope list_scope.

Inductive value :=
| Whole_value (n : Z)
| Bool_value (b : bool)
| Text_value (t : string).

(* The variables in use and their values, innermost first. *)
Definition environment := list (string * value).

Definition lookup (name : string) (env : environment) : option value :=
  match find (fun entry => String.eqb (fst entry) name) env with
  | Some (_, v) => Some v
  | None => None
  end.

(* Gives a variable a new value (D14), where it was declared. *)
Fixpoint assign (name : string) (v : value) (env : environment) : option environment :=
  match env with
  | [] => None
  | (n, old) :: rest =>
      if String.eqb n name then Some ((n, v) :: rest)
      else match assign name v rest with
           | Some rest' => Some ((n, old) :: rest')
           | None => None
           end
  end.

(* The variables a block declared end with it (D93): keeps only as many as were in use before it. *)
Definition restore (before after : environment) : environment :=
  skipn (length after - length before) after.

(* How a statement or block finished: carrying on, leaving the innermost loop with `break` (D32), or leaving the
   procedure with `return` and maybe a value (D57). *)
Inductive flow :=
| Continue (env : environment)
| Leave_loop (env : environment)
| Leave_procedure (result : option value).

Definition restore_flow (before : environment) (f : flow) : flow :=
  match f with
  | Continue after => Continue (restore before after)
  | Leave_loop after => Leave_loop (restore before after)
  | Leave_procedure result => Leave_procedure result
  end.

(* What a call gives back: a value, nothing, or a failure (D23). *)
Inductive outcome := Gave (v : value) | Gave_nothing | Failed.

(* trace: D46, D92 *)
Definition checked_whole {A : Type} (at_ : place) (result : checked) (operation : arithmetic_operation)
    (make : Z -> A) : io A :=
  match result with
  | Fits n => Done (make n)
  | Too_big => Stop (Overflow at_ true operation)
  | Too_small => Stop (Overflow at_ false operation)
  end.

Definition arithmetic_of (operator : binary_operator) : option arithmetic :=
  match operator with
  | Add => Some Plus
  | Subtract => Some Minus
  | Multiply => Some Times
  | _ => None
  end.

(* An operator on two values already worked out; `and` and `or` are handled before, since they stop early (D34). *)
Definition apply_binary (operator : binary_operator) (a b : value) (at_ : place) : io value :=
  match operator, a, b with
  | (Add | Subtract | Multiply), Whole_value x, Whole_value y =>
      match arithmetic_of operator with
      | Some op => checked_whole at_ (whole_arithmetic op x y) (Operation_binary x op y) Whole_value
      | None => Stop Stuck
      end
  | Join, Text_value x, Text_value y => Done (Text_value (x ++ y)%string)
  | Equal, Whole_value x, Whole_value y => Done (Bool_value (Z.eqb x y))
  | Equal, Bool_value x, Bool_value y => Done (Bool_value (Bool.eqb x y))
  | Not_equal, Whole_value x, Whole_value y => Done (Bool_value (negb (Z.eqb x y)))
  | Not_equal, Bool_value x, Bool_value y => Done (Bool_value (negb (Bool.eqb x y)))
  | Less, Whole_value x, Whole_value y => Done (Bool_value (Z.ltb x y))
  | Greater, Whole_value x, Whole_value y => Done (Bool_value (Z.gtb x y))
  | At_least, Whole_value x, Whole_value y => Done (Bool_value (Z.geb x y))
  | At_most, Whole_value x, Whole_value y => Done (Bool_value (Z.leb x y))
  | _, _, _ => Stop Stuck
  end.

(* trace: D29. Fills in `.fmt` text's braces from the variables in use; a number becomes text by itself. *)
Fixpoint fill (env : environment) (pieces : list piece) : option string :=
  match pieces with
  | [] => Some ""%string
  | p :: rest =>
      let this :=
        match p with
        | Piece_text t => Some t
        | Piece_variable name _ =>
            match lookup name env with
            | Some (Whole_value n) => Some (text_of_whole n)
            | Some (Text_value t) => Some t
            | _ => None
            end
        | _ => None
        end in
      match this, fill env rest with
      | Some a, Some b => Some (a ++ b)%string
      | _, _ => None
      end
  end.

(* The text `.fmt` is given, filled in. *)
Definition format_text (env : environment) (given : arguments) : option string :=
  match given with
  | Argument None (Text_literal _ pieces _) No_arguments => fill env pieces
  | _ => None
  end.

(* A procedure's parameters, given the arguments' values in order. *)
Fixpoint bind_parameters (parameters : list parameter) (values : list value) : option environment :=
  match parameters, values with
  | [], [] => Some []
  | p :: ps, v :: vs =>
      match bind_parameters ps vs with
      | Some env => Some (env ++ [(parameter_name p, v)])
      | None => None
      end
  | _, _ => None
  end.

(* The library's procedures (chapter 9 and D6, D20, D22, D26, D29, D95, D96). *)
Definition apply_library (env : environment) (which : library) (values : list value) (given : arguments)
    (at_ : place) : io outcome :=
  match which, values with
  | Show_message false same_line, [Text_value t] => Show t (negb same_line) (fun _ => Done Gave_nothing)
  | Show_message true same_line, _ =>
      match format_text env given with
      | Some t => Show t (negb same_line) (fun _ => Done Gave_nothing)
      | None => Stop Stuck
      end
  | Convert_to_text false, [Whole_value n] => Done (Gave (Text_value (text_of_whole n)))
  | Convert_to_text true, _ =>
      match format_text env given with
      | Some t => Done (Gave (Text_value t))
      | None => Stop Stuck
      end
  | Convert_to_whole, [Text_value t] =>
      match whole_of_text t with
      | Some n => Done (Gave (Whole_value n))
      | None => Done Failed
      end
  | Ask_for_line, [Text_value question] =>
      Ask question (fun answer =>
        match answer with
        | Some line => Done (Gave (Text_value line))
        | None => Stop Input_ended
        end)
  | Random_number, [Whole_value low; Whole_value high] =>
      if (low <=? high)%Z then Random low high (fun n => Done (Gave (Whole_value n)))
      else Stop (Random_range_not_designed at_ low high)
  | _, _ => Stop Stuck
  end.

(* What a procedure of the program's own gives back, from how its body finished (D37, D38, D57). *)
Definition procedure_result (d : definition) (f : flow) : io outcome :=
  match f, definition_returns d with
  | Leave_procedure (Some v), Some _ => Done (Gave v)
  | Leave_procedure None, None => Done Gave_nothing
  | Continue _, None => Done Gave_nothing
  | _, _ => Stop Stuck
  end.

Definition need_value (o : outcome) : io value :=
  match o with
  | Gave v => Done v
  | _ => Stop Stuck
  end.

(* Picks a way by a `Bool`. Each way is given as a function, so the way not taken is never worked out: OCaml, which
   the core is extracted to, works out a function's arguments before calling it, and working out both ways of an `if`
   whose other way calls the procedure it's in would never end. *)
Definition need_bool {A : Type} (v : value) (then_ else_ : unit -> io A) : io A :=
  match v with
  | Bool_value true => then_ tt
  | Bool_value false => else_ tt
  | _ => Stop Stuck
  end.

Fixpoint evaluate (fuel : nat) (p : program) (env : environment) (e : expression) {struct fuel} : io value :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match e with
      | Whole_literal digits _ =>
          match whole_of_literal digits with
          | Some n => Done (Whole_value n)
          | None => Stop Stuck
          end
      | Text_literal t _ _ => Done (Text_value t)
      | Bool_literal b _ => Done (Bool_value b)
      | Variable_value name _ =>
          match lookup name env with
          | Some v => Done v
          | None => Stop Stuck
          end
      | Negate operand at_ =>
          bind (evaluate fuel' p env operand) (fun v =>
            match v with
            | Whole_value n => checked_whole at_ (whole_negate n) (Operation_negate n) Whole_value
            | _ => Stop Stuck
            end)
      | Not operand _ =>
          bind (evaluate fuel' p env operand) (fun v =>
            need_bool v (fun _ => Done (Bool_value false)) (fun _ => Done (Bool_value true)))
      (* trace: D34. `and` and `or` stop early: the right side runs only if it can still change the answer. *)
      | Binary And left_operand right_operand _ _ =>
          bind (evaluate fuel' p env left_operand) (fun a =>
            need_bool a
              (fun _ => bind (evaluate fuel' p env right_operand) (fun b =>
                 need_bool b (fun _ => Done (Bool_value true)) (fun _ => Done (Bool_value false))))
              (fun _ => Done (Bool_value false)))
      | Binary Or left_operand right_operand _ _ =>
          bind (evaluate fuel' p env left_operand) (fun a =>
            need_bool a
              (fun _ => Done (Bool_value true))
              (fun _ => bind (evaluate fuel' p env right_operand) (fun b =>
                 need_bool b (fun _ => Done (Bool_value true)) (fun _ => Done (Bool_value false)))))
      | Binary operator left_operand right_operand _ at_ =>
          bind (evaluate fuel' p env left_operand) (fun a =>
            bind (evaluate fuel' p env right_operand) (fun b => apply_binary operator a b at_))
      | Bracketed inner _ => evaluate fuel' p env inner
      | Call name given at_ => bind (call fuel' p env name given at_) need_value
      (* trace: D23. `??` carries on with the value after it when the call fails. *)
      | Fallback (Call name given call_at) fallback _ _ =>
          bind (call fuel' p env name given call_at) (fun o =>
            match o with
            | Gave v => Done v
            | Failed => evaluate fuel' p env fallback
            | Gave_nothing => Stop Stuck
            end)
      | Fallback _ _ _ _ => Stop Stuck
      (* trace: D5 *)
      | Slide start path _ =>
          bind (evaluate fuel' p env start) (fun v => bind (slide fuel' p env v path) need_value)
      | Bare_word _ _ | Broken _ => Stop Stuck
      end
  end

(* Works out each argument, left to right. *)
with evaluate_arguments (fuel : nat) (p : program) (env : environment) (given : arguments) {struct fuel}
  : io (list value) :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match given with
      | No_arguments => Done []
      | Argument _ value rest =>
          bind (evaluate fuel' p env value) (fun v =>
            bind (evaluate_arguments fuel' p env rest) (fun vs => Done (v :: vs)))
      end
  end

(* Runs the procedure [name] with [values] as its arguments, in order (D24: labels only say which is which). *)
with apply (fuel : nat) (p : program) (env : environment) (name : callee) (values : list value) (given : arguments)
    (at_ : place) {struct fuel} : io outcome :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match resolve p name with
      | Unresolved _ => Stop Stuck
      | Resolved (Library which) => apply_library env which values given at_
      | Resolved (User d) =>
          match bind_parameters (definition_parameters d) values with
          | Some locals => bind (execute_block fuel' p locals (definition_body d)) (procedure_result d)
          | None => Stop Stuck
          end
      end
  end

with call (fuel : nat) (p : program) (env : environment) (name : callee) (given : arguments) (at_ : place)
    {struct fuel} : io outcome :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      bind (evaluate_arguments fuel' p env given) (fun values => apply fuel' p env name values given at_)
  end

(* Passes [v] into each step as its first argument (D5). *)
with slide (fuel : nat) (p : program) (env : environment) (v : value) (path : steps) {struct fuel} : io outcome :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match path with
      | No_steps => Stop Stuck
      | Step name _ given at_ rest =>
          bind (evaluate_arguments fuel' p env given) (fun values =>
            bind (apply fuel' p env name (v :: values) given at_) (fun o =>
              match rest with
              | No_steps => Done o
              | Step _ _ _ _ _ =>
                  match o with
                  | Gave w => slide fuel' p env w rest
                  | _ => Stop Stuck
                  end
              end))
      end
  end

with execute (fuel : nat) (p : program) (env : environment) (st : statement) {struct fuel} : io flow :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match st with
      (* trace: D13 *)
      | Declare _ name _ _ _ value _ =>
          bind (evaluate fuel' p env value) (fun v => Done (Continue ((name, v) :: env)))
      (* trace: D14 *)
      | Assign name _ value _ _ =>
          bind (evaluate fuel' p env value) (fun v =>
            match assign name v env with
            | Some env' => Done (Continue env')
            | None => Stop Stuck
            end)
      (* trace: D120. A call standing alone gives nothing. *)
      | Run (Call name given at_) _ =>
          bind (call fuel' p env name given at_) (fun o =>
            match o with
            | Gave_nothing => Done (Continue env)
            | _ => Stop Stuck
            end)
      | Run (Slide start path _) _ =>
          bind (evaluate fuel' p env start) (fun v =>
            bind (slide fuel' p env v path) (fun o =>
              match o with
              | Gave_nothing => Done (Continue env)
              | _ => Stop Stuck
              end))
      | Run _ _ => Stop Stuck
      (* trace: D16 *)
      | If condition then_part rest _ =>
          bind (evaluate fuel' p env condition) (fun c =>
            need_bool c (fun _ => execute_block fuel' p env then_part) (fun _ => execute_else fuel' p env rest))
      (* trace: D19, D32 *)
      | While condition body at_ =>
          bind (evaluate fuel' p env condition) (fun c =>
            need_bool c
              (fun _ => bind (execute_block fuel' p env body) (fun f =>
                 match f with
                 | Continue env' => execute fuel' p env' (While condition body at_)
                 | Leave_loop env' => Done (Continue env')
                 | Leave_procedure result => Done (Leave_procedure result)
                 end))
              (fun _ => Done (Continue env)))
      | Break _ => Done (Leave_loop env)
      (* trace: D57 *)
      | Return None _ => Done (Leave_procedure None)
      | Return (Some value) _ => bind (evaluate fuel' p env value) (fun v => Done (Leave_procedure (Some v)))
      (* trace: D25. A `labels` line runs nothing. *)
      | Labels _ _ => Done (Continue env)
      end
  end

(* Runs a block's statements in turn, until one leaves. *)
with execute_statements (fuel : nat) (p : program) (env : environment) (b : block) {struct fuel} : io flow :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match b with
      | Empty => Done (Continue env)
      | Then st rest =>
          bind (execute fuel' p env st) (fun f =>
            match f with
            | Continue env' => execute_statements fuel' p env' rest
            | other => Done other
            end)
      end
  end

(* A block: its statements, then its own variables end (D93). *)
with execute_block (fuel : nat) (p : program) (env : environment) (b : block) {struct fuel} : io flow :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' => bind (execute_statements fuel' p env b) (fun f => Done (restore_flow env f))
  end

with execute_else (fuel : nat) (p : program) (env : environment) (e : else_part) {struct fuel} : io flow :=
  match fuel with
  | O => Stop Out_of_fuel
  | S fuel' =>
      match e with
      | No_else => Done (Continue env)
      | Else body _ => execute_block fuel' p env body
      | Else_if condition body rest _ =>
          bind (evaluate fuel' p env condition) (fun c =>
            need_bool c (fun _ => execute_block fuel' p env body) (fun _ => execute_else fuel' p env rest))
      end
  end.

(* THE EVALUATOR: runs a program from `proc.main` (D4), with [fuel] steps at most. *)
Definition run (fuel : nat) (p : program) : io unit :=
  match find_main p with
  | Some d =>
      bind (execute_block fuel p [] (definition_body d)) (fun f =>
        match f with
        | Continue _ | Leave_procedure None => Done tt
        | _ => Stop Stuck
        end)
  | None => Stop Stuck
  end.
