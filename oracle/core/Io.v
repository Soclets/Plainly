(* How the pure core asks for input and output.

   The evaluator never prints or reads by itself. It gives back a tree of requests: "show this text", "I need a line
   of input", "I need a random number from low to high", each holding what to do next with the answer. The shell
   (shell/ralph.ml) walks the tree and does the real printing, reading and random numbers, so everything in Rocq stays
   pure and provable. *)

From Stdlib Require Import String ZArith.
From Ralph Require Import Syntax Whole.

(* Why a program stopped before reaching its end. *)
Inductive arithmetic_operation :=
| Operation_binary (a : Z) (operator : arithmetic) (b : Z)
| Operation_negate (a : Z).

Inductive stop :=
(* D46's overflow check failed, at a place, with the values that broke it (D92). *)
| Overflow (at_ : place) (too_big : bool) (operation : arithmetic_operation)
(* trace: D154. Calls nested too deep: how deep, and the procedure the call that went too deep is in. *)
| Calls_too_deep (at_ : place) (depth : nat) (inside : string)
(* `ask` found no more input: the program ends quietly with status 0 (D96). *)
| Input_ended
(* `random` with `from:` above `to:`, which isn't decided (D26). *)
| Random_range_not_designed (at_ : place) (low high : Z)
(* The evaluator ran out of fuel, its step counter (see Evaluator.v). *)
| Out_of_fuel
(* The evaluator met something it has no rule for. The type-safety theorem says a program the checker accepts
   never reaches this. *)
| Stuck.

Inductive io (A : Type) : Type :=
| Done (result : A)
| Stop (reason : stop)
| Show (text : string) (ends_line : bool) (next : unit -> io A)
| Ask (question : string) (next : option string -> io A)
| Random (low high : Z) (next : Z -> io A).

Arguments Done {A}.
Arguments Stop {A}.
Arguments Show {A}.
Arguments Ask {A}.
Arguments Random {A}.

(* Does [m], then [k] with its result. *)
Fixpoint bind {A B : Type} (m : io A) (k : A -> io B) : io B :=
  match m with
  | Done a => k a
  | Stop reason => Stop reason
  | Show text ends_line next => Show text ends_line (fun u => bind (next u) k)
  | Ask question next => Ask question (fun answer => bind (next answer) k)
  | Random low high next => Random low high (fun n => bind (next n) k)
  end.

(* [safe P m]: however the input goes, and whatever random numbers come back, every way through [m] ends in a
   result satisfying [P], or a stop that isn't [Stuck]. Each way through the tree is finite, so this is an ordinary
   inductive definition. *)
Inductive safe {A : Type} (P : A -> Prop) : io A -> Prop :=
| safe_done : forall a, P a -> safe P (Done a)
| safe_stop : forall reason, reason <> Stuck -> safe P (Stop reason)
| safe_show : forall text ends_line next, safe P (next tt) -> safe P (Show text ends_line next)
| safe_ask : forall question next, (forall answer, safe P (next answer)) -> safe P (Ask question next)
| safe_random : forall low high next, (forall n, safe P (next n)) -> safe P (Random low high next).

Lemma safe_bind : forall (A B : Type) (P : A -> Prop) (Q : B -> Prop) (m : io A) (k : A -> io B),
  safe P m -> (forall a, P a -> safe Q (k a)) -> safe Q (bind m k).
Proof.
  intros A B P Q m k Hm Hk; induction Hm; simpl; try (apply Hk; assumption); constructor; auto.
Qed.

Lemma safe_weaken : forall (A : Type) (P Q : A -> Prop) (m : io A),
  safe P m -> (forall a, P a -> Q a) -> safe Q m.
Proof.
  intros A P Q m Hm HPQ; induction Hm; constructor; auto.
Qed.
