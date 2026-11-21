(* Type safety: a program the checker accepts never gets stuck.

   The evaluator (Evaluator.v) stops with [Stuck] whenever it meets something it has no rule for: a variable that
   isn't there, `+` on a `Bool`, a procedure that doesn't exist, `break` outside a loop, a procedure with `returns`
   finishing without one, and so on. This file proves that none of that ever happens to a program the checker
   (Checker.v) accepts, however its input goes and whatever random numbers come back.

   The proof is by induction on the evaluator's fuel, the "definitional interpreter" method: at each amount of fuel,
   every part of an accepted program, run where the checker said its variables would be, gives a value of the type the
   checker worked out, or stops for a reason that isn't [Stuck]. *)

From Stdlib Require Import String List Bool ZArith Lia.
From Ralph Require Import Syntax Whole Io Diagnostics Procedures Checker Evaluator.
Import ListNotations.
Open Scope list_scope.
Open Scope nat_scope.

(* ---------------------------------------------------------------------------------------------------------------- *)
(* Values, variables and their types *)

Definition type_of_value (v : value) : type :=
  match v with
  | Whole_value _ => Whole
  | Bool_value _ => Bool
  | Text_value _ => Text
  end.

Definition has_type (t : type) (v : value) : Prop := type_of_value v = t.

(* The values in [env] are where and of the types the checker's [scope] said. *)
Definition matches (scope : list variable_info) (env : environment) : Prop :=
  Forall2 (fun info entry => variable_name info = fst entry /\ variable_type info = type_of_value (snd entry))
    scope env.

Lemma type_eqb_true : forall a b, type_eqb a b = true -> a = b.
Proof. intros [] []; simpl; congruence. Qed.

Lemma lookup_matches : forall scope env name info,
  matches scope env -> find_variable name scope = Some info ->
  exists v, lookup name env = Some v /\ type_of_value v = variable_type info.
Proof.
  intros scope env name info H; induction H as [| i [n v] scope' env' [Hn Ht] _ IH]; simpl in *.
  - discriminate.
  - unfold find_variable, lookup in *; simpl in *. subst n.
    destruct (String.eqb (variable_name i) name); intros E.
    + inversion E; subst; eauto.
    + apply IH; assumption.
Qed.

Lemma assign_matches : forall scope env name info v,
  matches scope env -> find_variable name scope = Some info -> type_of_value v = variable_type info ->
  exists env', assign name v env = Some env' /\ matches scope env'.
Proof.
  intros scope env name info v H; induction H as [| i [n w] scope' env' [Hn Ht] Hrest IH]; simpl in *.
  - discriminate.
  - unfold find_variable in *; simpl in *. subst n.
    destruct (String.eqb (variable_name i) name) eqn:E; intros F Tv.
    + inversion F; subst. eexists; split; [reflexivity|]. constructor; simpl; auto.
    + destruct (IH F Tv) as [env'' [A M]]. rewrite A. eexists; split; [reflexivity|]. constructor; simpl; auto.
Qed.

Lemma restore_matches : forall scope extra env env',
  matches scope env -> matches (extra ++ scope) env' -> matches scope (restore env env').
Proof.
  intros scope extra env env' H H'.
  unfold matches in *.
  apply Forall2_app_inv_l in H'. destruct H' as [l1 [l2 [H1 [H2 E]]]]. subst env'.
  unfold restore. apply Forall2_length in H. pose proof (Forall2_length H2) as L2.
  rewrite length_app. replace (length l1 + length l2 - length env) with (length l1) by lia.
  rewrite skipn_app, skipn_all, Nat.sub_diag; simpl. assumption.
Qed.

Lemma restore_same : forall scope env env',
  matches scope env -> matches scope env' -> matches scope (restore env env').
Proof. intros. apply (restore_matches scope [] env env'); auto. Qed.

Lemma parameters_match : forall parameters values,
  Forall2 (fun p v => parameter_type p = type_of_value v) parameters values ->
  exists locals, bind_parameters parameters values = Some locals /\
                 matches (rev (map parameter_info parameters)) locals.
Proof.
  intros parameters values H; induction H as [| p v ps vs Hpv _ [locals [B M]]]; simpl.
  - exists []; split; [reflexivity | constructor].
  - rewrite B. eexists; split; [reflexivity|].
    unfold matches. apply Forall2_app; [assumption|]. constructor; [|constructor]. simpl. auto.
Qed.

(* ---------------------------------------------------------------------------------------------------------------- *)
(* An expression whose type is unknown always comes with something said about it. *)

Create HintDb reported.

Lemma app_nil : forall (A : Type) (l1 l2 : list A), l1 ++ l2 = [] -> l1 = [] /\ l2 = [].
Proof. intros A [|x l1] l2; simpl; intros H; [auto | discriminate]. Qed.
Lemma app_not_nil_r : forall (A : Type) (l1 l2 : list A), l2 <> [] -> l1 ++ l2 <> [].
Proof. intros A l1 l2 H E. apply app_nil in E. tauto. Qed.
Lemma app_not_nil_l : forall (A : Type) (l1 l2 : list A), l1 <> [] -> l1 ++ l2 <> [].
Proof. intros A l1 l2 H E. apply app_nil in E. tauto. Qed.
Lemma cons_not_nil : forall (A : Type) (x : A) l, x :: l <> [].
Proof. discriminate. Qed.
#[local] Hint Resolve app_not_nil_r app_not_nil_l cons_not_nil : reported.
Lemma resolve_reported : forall p c problems, resolve p c = Unresolved problems -> problems <> [].
Proof.
  intros p c problems.
  unfold resolve, resolve_show, resolve_be, plain.
  repeat match goal with
  | |- context [if ?b then _ else _] => destruct b
  | |- context [match ?x with _ => _ end] => destruct x
  end; intros H; inversion H; subst; try discriminate.
Qed.
Definition reported (a : checked_argument) : Prop :=
  argument_type a = None -> argument_diagnostics a <> [].

Lemma single_reported : forall procedure args at_ d, Forall reported args ->
  check_single procedure args at_ = (None, d) -> d <> [].
Proof.
  intros procedure args at_ d R; unfold check_single.
  destruct args as [|a [|b rest]]; simpl; intros H; inversion H; subst; auto with reported.
  inversion R; subst.
  destruct (argument_label a) as [[? ?]|]; [inversion H; subst; auto with reported|].
  destruct (argument_type a) eqn:T; inversion H; subst; auto.
Qed.

Lemma call_reported : forall s name args at_, Forall reported args ->
  call_gives (check_call s name args at_) = Gives_unknown -> call_diagnostics (check_call s name args at_) <> [].
Proof.
  intros s name args at_ R.
  unfold check_call. destruct (resolve (setting_program s) name) as [target | problems] eqn:Res; simpl.
  - destruct target as [which | d]; [destruct which as [[] ? | [] | | |] |]; simpl;
      unfold check_format, gives_of, make_call, unknown_call; simpl; try discriminate;
      repeat match goal with
      | |- context [match ?x with _ => _ end] => destruct x eqn:?
      end; simpl; try discriminate; intros _; auto with reported;
      try (apply app_not_nil_l; inversion R; subst; auto).
    all: try (match goal with H : check_single _ _ _ = (None, _) |- _ =>
                eapply single_reported; [|exact H]; assumption end).
    all: try (apply app_not_nil_r; discriminate).
  - intros _. apply app_not_nil_l. apply app_not_nil_l. eapply resolve_reported; eauto.
Qed.

Lemma step_reported : forall s name slid args at_, Forall reported args ->
  call_gives (check_step s name slid args at_) = Gives_unknown -> call_diagnostics (check_step s name slid args at_) <> [].
Proof.
  intros s name slid args at_ R.
  unfold check_step. destruct (resolve (setting_program s) name) as [target | problems] eqn:Res; simpl.
  - destruct target as [which | d]; [destruct which as [[] ? | [] | | |] |]; simpl;
      unfold check_step_target, gives_of, make_call, unknown_call; simpl;
      repeat match goal with
      | |- context [match ?x with _ => _ end] => destruct x eqn:?
      end; simpl; try discriminate; intros _; auto with reported.
  - intros _. apply app_not_nil_l. eapply resolve_reported; eauto.
Qed.

Lemma as_value_reported : forall c at_,
  (call_gives c = Gives_unknown -> call_diagnostics c <> []) ->
  fst (as_value c at_) = None -> snd (as_value c at_) <> [].
Proof.
  intros c at_ H; unfold as_value.
  destruct (call_gives c), (call_can_fail c); simpl; try discriminate; intros _; auto with reported.
Qed.

Lemma binary_type_reported : forall operator a b l r at_ d,
  binary_type operator a b l r at_ = (None, d) -> d <> [].
Proof.
  intros operator a b l r at_ d; unfold binary_type.
  destruct operator, a, b; simpl; intros H; inversion H; subst; auto with reported.
Qed.

Lemma slide_shape_empty : forall start at_, slide_shape start No_steps at_ <> [].
Proof.
  intros start at_; unfold slide_shape, count_steps.
  destruct (2 <=? chain_depth start)%nat eqn:E; [intro X; discriminate X|].
  rewrite Nat.add_0_r. apply Nat.leb_gt in E.
  destruct (chain_depth start <? 3)%nat eqn:F; [intro X; discriminate X|].
  apply Nat.ltb_ge in F; lia.
Qed.

Lemma unknown_reported :
  (forall e s, fst (check_expression s e) = None -> snd (check_expression s e) <> []) /\
  (forall given s, Forall reported (check_arguments s given)) /\
  (forall path s slid, path <> No_steps -> call_gives (check_steps s slid path) = Gives_unknown ->
     call_diagnostics (check_steps s slid path) <> []).
Proof.
  apply expression_induction.
  - (* Whole_literal *) intros digits at_ s; simpl.
    destruct (whole_of_literal digits); [destruct (_ && _)|]; simpl; try discriminate; auto with reported.
  - intros; simpl; discriminate.
  - intros; simpl; discriminate.
  - intros name at_ s; simpl. destruct (find_variable _ _); simpl; [discriminate | auto with reported].
  - intros word at_ s; simpl. destruct (is_library_word word); [simpl; auto with reported|].
    destruct (find_definition _ _); simpl; auto with reported.
  - (* Negate *) intros o IH at_ s; simpl. specialize (IH s).
    destruct (check_expression s o) as [[[]|] ds]; simpl in *; try discriminate; auto with reported.
  - (* Not *) intros o IH at_ s; simpl. specialize (IH s).
    destruct (check_expression s o) as [[[]|] ds]; simpl in *; try discriminate; auto with reported.
  - (* Binary *) intros operator l IHl r IHr operator_at at_ s; simpl. specialize (IHl s); specialize (IHr s).
    destruct (check_expression s l) as [a dl], (check_expression s r) as [b dr]; simpl in *.
    destruct (beside_maths _ _ _ _); simpl; [auto with reported|].
    destruct a, b; simpl; auto with reported.
    destruct (binary_type _ _ _ _ _ _) as [[]] eqn:E; simpl; [discriminate|].
    intros _. apply app_not_nil_r, app_not_nil_r. eapply binary_type_reported; eauto.
  - (* Bracketed *) intros inner IH at_ s; simpl; auto.
  - (* Call *) intros name given IH at_ s; simpl. apply as_value_reported. apply call_reported. apply IH.
  - (* Fallback *) intros attempt IHa fallback IHf operator_at at_ s. cbn [check_expression].
    destruct (check_expression s fallback) as [f df] eqn:F.
    destruct attempt as [ | | | | | | | | | name given call_at | | | ]; cbn beta iota zeta;
      try (match goal with |- context [check_expression ?a ?b] => destruct (check_expression a b) end;
           simpl; intros; auto with reported; fail).
    specialize (IHa s); cbn [check_expression] in IHa.
    destruct (check_call s name (check_arguments s given) call_at) as [g fails diagnostics] eqn:C; simpl in *.
    destruct g, fails; simpl; try discriminate; intros _; auto with reported.
  - (* Slide *) intros start IHs path IHp at_ s; simpl.
    destruct (check_expression s start) as [t ds]; simpl.
    destruct (as_value (check_steps s t path) at_) as [result dc] eqn:A; simpl.
    intros N; subst result.
    destruct path as [| name bracketed given step_at rest].
    + apply app_not_nil_r, app_not_nil_l, slide_shape_empty.
    + apply app_not_nil_r, app_not_nil_r.
      assert (snd (as_value (check_steps s t (Step name bracketed given step_at rest)) at_) <> []) as H.
      { apply as_value_reported; [|rewrite A; reflexivity].
        intros G. apply IHp; [discriminate | assumption]. }
      rewrite A in H. exact H.
  - intros at_ s; simpl; auto with reported.
  - intros s; simpl; constructor.
  - intros label value IHv rest IHr s; simpl.
    destruct (check_expression s value) as [t d] eqn:E. constructor; [|apply IHr].
    unfold reported; simpl. intros T; subst t. specialize (IHv s). rewrite E in IHv. apply IHv; reflexivity.
  - intros s slid N; exfalso; apply N; reflexivity.
  - intros name bracketed given IHg at_ rest IHr s slid _; simpl.
    destruct rest as [| name2 bracketed2 given2 at2 rest2].
    + apply step_reported, IHg.
    + destruct (as_value _ at_) as [next d]; simpl. intros G.
      apply app_not_nil_r. apply IHr; [discriminate | assumption].
Qed.
