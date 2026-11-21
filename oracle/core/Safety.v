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
  - intros _. apply app_not_nil_l. eapply resolve_reported; eauto.
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
  - (* Call *) intros name given IH at_ s; cbn [check_expression].
    pose proof (as_value_reported (check_call s name (check_arguments s given) at_) at_) as A.
    destruct (as_value _ at_) as [t d]; simpl in *. intros T. apply app_not_nil_l. apply A; auto.
    apply call_reported. apply IH.
  - (* Fallback *) intros attempt IHa fallback IHf operator_at at_ s. cbn [check_expression].
    destruct (check_expression s fallback) as [f df] eqn:F.
    destruct attempt as [ | | | | | | | | | name given call_at | | | ]; cbn beta iota zeta;
      try (match goal with |- context [check_expression ?a ?b] => destruct (check_expression a b) end;
           simpl; intros; auto with reported; fail).
    specialize (IHa s); cbn [check_expression] in IHa.
    destruct (check_call s name (check_arguments s given) call_at) as [g fails diagnostics] eqn:C; simpl in *.
    destruct g, fails; simpl in *; try discriminate; intros _; auto with reported;
      rewrite app_assoc; apply app_not_nil_l; apply IHa; reflexivity.
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

(* ---------------------------------------------------------------------------------------------------------------- *)
(* What a well-typed result is, for calls, statements and blocks *)

Definition outcome_ok (g : gives) (can_fail : bool) (o : outcome) : Prop :=
  match g, o with
  | Gives_value t, Gave v => type_of_value v = t
  | Gives_value _, Failed => can_fail = true
  | Gives_nothing, Gave_nothing => True
  | _, _ => False
  end.

Definition result_ok (returns : option type) (r : option value) : Prop :=
  match returns, r with
  | Some t, Some v => type_of_value v = t
  | None, None => True
  | _, _ => False
  end.

Definition flow_ok (s s' : setting) (always : bool) (f : flow) : Prop :=
  match f with
  | Continue env' => always = false /\ matches (setting_scope s') env'
  | Leave_loop env' => setting_in_loop s = true /\ matches (setting_scope s) env'
  | Leave_procedure r => result_ok (setting_returns s) r
  end.

Definition block_ok (s : setting) (always : bool) (f : flow) : Prop :=
  match f with
  | Continue env' => always = false /\ exists extra, matches (extra ++ setting_scope s) env'
  | Leave_loop env' => setting_in_loop s = true /\ exists extra, matches (extra ++ setting_scope s) env'
  | Leave_procedure r => result_ok (setting_returns s) r
  end.

Lemma not_stuck_overflow : forall a b c, Overflow a b c <> Stuck. Proof. discriminate. Qed.
#[local] Hint Resolve not_stuck_overflow : core.

Lemma safe_checked_whole : forall at_ r op,
  safe (has_type Whole) (checked_whole at_ r op Whole_value).
Proof. intros at_ [] op; simpl; constructor; try discriminate; reflexivity. Qed.

Lemma safe_need_bool : forall (A : Type) (P : A -> Prop) v (a b : io A),
  type_of_value v = Bool -> safe P a -> safe P b -> safe P (need_bool v (fun _ => a) (fun _ => b)).
Proof. intros A P [| [] |] a b T Ha Hb; simpl in *; try discriminate; assumption. Qed.

Lemma safe_apply_binary : forall operator a b l r operator_at t x y at_,
  operator <> And -> operator <> Or ->
  binary_type operator a b l r operator_at = (Some t, []) ->
  type_of_value x = a -> type_of_value y = b ->
  safe (has_type t) (apply_binary operator x y at_).
Proof.
  intros operator a b l r operator_at t x y at_ NA NO B Tx Ty.
  destruct operator; try congruence; destruct x, y; simpl in *; subst a b; simpl in B;
    inversion B; subst; try (constructor; reflexivity); try apply safe_checked_whole.
Qed.

Lemma flat_map_nil : forall (A B : Type) (f : A -> list B) l x, flat_map f l = [] -> In x l -> f x = [].
Proof.
  intros A B f l x; induction l as [|y l IH]; simpl; [tauto|].
  intros E [H|H]; apply app_nil in E; destruct E; subst; auto.
Qed.

Lemma find_definition_in : forall p name d, find_definition p name = Some d -> In d p.
Proof. intros p name d H. apply find_some in H. tauto. Qed.

Lemma resolve_user_in : forall p name d, resolve p name = Resolved (User d) -> In d p.
Proof.
  intros p name d; unfold resolve, resolve_show, resolve_be, plain.
  repeat match goal with
  | |- context [if ?b then _ else _] => destruct b
  | |- context [match ?x with _ => _ end] => destruct x eqn:?
  end; intros H; inversion H; subst; try discriminate.
  all: eapply find_definition_in; eassumption.
Qed.

Definition target_gives (t : target) : gives :=
  match t with
  | Library (Show_message _ _) => Gives_nothing
  | Library (Convert_to_text _) => Gives_value Text
  | Library Convert_to_whole => Gives_value Whole
  | Library Ask_for_line => Gives_value Text
  | Library Random_number => Gives_value Whole
  | User d => gives_of d
  end.

Definition target_can_fail (t : target) : bool :=
  match t with Library Convert_to_whole => true | _ => false end.

Definition format_ready (s : setting) (given : arguments) : Prop :=
  exists v pieces at_, given = Argument None (Text_literal v pieces at_) No_arguments /\
                       Forall (fun pc => check_piece s pc = []) pieces.

Definition target_ready (s : setting) (t : target) (values : list value) (given : arguments) : Prop :=
  match t with
  | Library (Show_message true _) | Library (Convert_to_text true) => format_ready s given
  | Library (Show_message false _) | Library Ask_for_line | Library Convert_to_whole =>
      map type_of_value values = [Text]
  | Library (Convert_to_text false) => map type_of_value values = [Whole] \/ map type_of_value values = [Bool]
  | Library Random_number => map type_of_value values = [Whole; Whole]
  | User d => map type_of_value values = map parameter_type (definition_parameters d)
  end.

Definition typed (args : list checked_argument) (values : list value) : Prop :=
  Forall2 (fun a v => argument_type a = Some (type_of_value v)) args values.

Lemma check_pairs_typed : forall procedure parameters args values,
  length args = length parameters -> check_pairs procedure parameters args = [] -> typed args values ->
  map type_of_value values = map fst parameters.
Proof.
  intros procedure parameters args values L C T; revert parameters L C.
  induction T as [| a v args values Tv T IH]; intros [| [expected wanted] parameters] L C; simpl in *;
    try discriminate; auto.
  apply app_nil in C as [_ C]. apply app_nil in C as [_ C]. apply app_nil in C as [E C].
  rewrite Tv in E. unfold expect in E. destruct (type_eqb _ _) eqn:Q; [|discriminate].
  apply type_eqb_true in Q. rewrite Q. f_equal. apply IH; auto.
Qed.

Lemma check_against_typed : forall procedure parameters args at_ declared values,
  check_against procedure parameters args at_ declared = [] -> typed args values ->
  map type_of_value values = map fst parameters.
Proof.
  intros procedure parameters args at_ declared values; unfold check_against.
  destruct (negb _) eqn:N; [intros C; apply app_nil in C; destruct C as [_ C]; discriminate|].
  destruct (labels_reordered _ _); [intros C; apply app_nil in C; destruct C as [_ C]; discriminate|].
  intros C T. apply negb_false_iff, Nat.eqb_eq in N. eapply check_pairs_typed; eauto.
Qed.

Lemma check_arguments_one : forall s given a, check_arguments s given = [a] ->
  given = Argument (argument_label a) (argument_value a) No_arguments.
Proof.
  intros s [| label value [| l2 v2 r2]] a; simpl; try discriminate.
  - destruct (check_expression s value); intros H; inversion H; reflexivity.
  - destruct (check_expression s value), (check_expression s v2); intros H; inversion H.
Qed.

Lemma check_format_ready : forall s given at_,
  check_format s (check_arguments s given) at_ = [] -> format_ready s given.
Proof.
  intros s given at_; unfold check_format.
  destruct (check_arguments s given) as [| a [| b rest]] eqn:A; try (intros C; apply app_nil in C; destruct C; discriminate).
  destruct (argument_label a) as [[? ?]|] eqn:La; [intros C; apply app_nil in C; destruct C; discriminate|].
  destruct (argument_value a) eqn:Va; try (intros C; apply app_nil in C; destruct C; discriminate).
  intros C. apply app_nil in C as [_ C].
  apply check_arguments_one in A. rewrite La, Va in A.
  do 3 eexists; split; [exact A|].
  apply Forall_forall. intros x X. eapply flat_map_nil; eauto.
Qed.

Lemma typed_one : forall a values t, typed [a] values -> argument_type a = Some t ->
  map type_of_value values = [t].
Proof.
  intros a values t T A. inversion T as [| a' v ? ? Tv T']; subst. inversion T'; subst.
  rewrite A in Tv. inversion Tv. reflexivity.
Qed.

Ltac single_case :=
  match goal with
  | |- context [check_single ?procedure (check_arguments ?s ?given) ?at_] =>
      destruct (check_single procedure (check_arguments s given) at_) as [[[[] ?]|] d] eqn:Sg; simpl in *;
      try (match goal with C : _ ++ _ = [] |- _ => apply app_nil in C; destruct C as [_ C]; discriminate C end);
      try (exfalso; refine (single_reported _ _ _ _ (proj1 (proj2 unknown_reported) given s) Sg _); assumption);
      repeat split; unfold check_single in Sg;
      destruct (check_arguments s given) as [| a [| ? ?]]; try discriminate;
      destruct (argument_label a) as [[? ?]|]; try discriminate;
      destruct (argument_type a) eqn:Ta; inversion Sg; subst;
      first [solve [eapply typed_one; eauto] | solve [left; eapply typed_one; eauto]
            | solve [right; eapply typed_one; eauto]]
  end.

Lemma call_ready : forall s name given at_ values,
  call_diagnostics (check_call s name (check_arguments s given) at_) = [] ->
  typed (check_arguments s given) values ->
  exists target, resolve (setting_program s) name = Resolved target /\
    target_ready s target values given /\
    call_gives (check_call s name (check_arguments s given) at_) = target_gives target /\
    call_can_fail (check_call s name (check_arguments s given) at_) = target_can_fail target.
Proof.
  intros s name given at_ values.
  unfold check_call. destruct (resolve (setting_program s) name) as [target | problems] eqn:Res; simpl.
  2:{ intros C. apply app_nil in C as [C _]. exfalso; eapply resolve_reported; eauto. }
  intros C T. exists target. split; [reflexivity|].
  destruct target as [which | d]; [destruct which as [[] ? | [] | | |] |]; simpl in *.
  - repeat split. eapply check_format_ready; eauto.
  - repeat split. eapply (check_against_typed _ [(Text, None)]); eauto.
  - repeat split. eapply check_format_ready; eauto.
  - single_case.
  - single_case.
  - repeat split. eapply (check_against_typed _ [(Text, None)]); eauto.
  - repeat split. eapply (check_against_typed _ [(Whole, Some "from"%string); (Whole, Some "to"%string)]); eauto.
  - repeat split. erewrite check_against_typed; eauto. unfold user_parameters. rewrite map_map. reflexivity.
Qed.

Ltac nonempty :=
  match goal with
  | C : _ ++ _ = [] |- _ => apply app_nil in C; destruct C as [_ C]; discriminate C
  | C : [] ++ _ = [] |- _ => simpl in C; discriminate C
  end.

Lemma step_ready : forall s name v given at_ values,
  call_diagnostics (check_step s name (Some (type_of_value v)) (check_arguments s given) at_) = [] ->
  typed (check_arguments s given) values ->
  exists target, resolve (setting_program s) name = Resolved target /\
    target_ready s target (v :: values) given /\
    call_gives (check_step s name (Some (type_of_value v)) (check_arguments s given) at_) = target_gives target /\
    call_can_fail (check_step s name (Some (type_of_value v)) (check_arguments s given) at_) = target_can_fail target.
Proof.
  intros s name v given at_ values.
  unfold check_step. destruct (resolve (setting_program s) name) as [target | problems] eqn:Res; simpl.
  2:{ intros C. apply app_nil in C as [C _]. exfalso; eapply resolve_reported; eauto. }
  intros C T. exists target. split; [reflexivity|].
  unfold check_step_target in *.
  assert (forall first others gives can_fail,
            call_diagnostics (match (first, None) :: others with
                              | [] => unknown_call []
                              | (f, _) :: os => make_call gives can_fail
                                  (expect (Some (type_of_value v)) f
                                     (fun found => Refused (Slid_value_type (callee_name name) f found at_))
                                   ++ check_against (callee_name name) os (check_arguments s given) at_ None)
                              end) = [] ->
            type_of_value v = first /\ map type_of_value values = map fst others) as One.
  { intros first others gives can_fail D; simpl in D. apply app_nil in D as [E D]. unfold expect in E.
    destruct (type_eqb _ _) eqn:Q; [|discriminate E]. apply type_eqb_true in Q.
    split; [assumption|]. eapply check_against_typed; eauto. }
  destruct target as [which | d]; [destruct which as [[] ? | [] | | |] |]; simpl in *.
  - nonempty.
  - destruct (One Text [] Gives_nothing false C) as [A B]. simpl in *. repeat split; simpl; congruence.
  - nonempty.
  - destruct (type_of_value v) eqn:Tv; simpl in *; try nonempty.
    all: try (apply app_nil in C as [_ C]); erewrite check_against_typed by eauto; simpl;
      repeat split; first [congruence | left; congruence | right; congruence].
  - destruct (type_of_value v) eqn:Tv; simpl in *; try nonempty.
    try (apply app_nil in C as [_ C]). erewrite check_against_typed by eauto. simpl. repeat split; congruence.
  - destruct (One Text [] Gives_nothing false C) as [A B]. simpl in *. repeat split; simpl; congruence.
  - destruct (One Whole [(Whole, Some "to"%string)] Gives_nothing false C) as [A B]. simpl in *. repeat split; simpl; congruence.
  - unfold user_parameters in *. destruct (definition_parameters d) as [| first others] eqn:P; simpl in *;
      [nonempty|].
    apply app_nil in C as [E C]. unfold expect in E. destruct (type_eqb _ _) eqn:Q; [|discriminate E].
    apply type_eqb_true in Q. repeat split; simpl.
    erewrite check_against_typed; [| eassumption | eassumption]. rewrite map_map. simpl. rewrite Q; reflexivity.
Qed.

Lemma fill_ok : forall s env pieces,
  matches (setting_scope s) env -> Forall (fun pc => check_piece s pc = []) pieces ->
  exists text, fill env pieces = Some text.
Proof.
  intros s env pieces M F; induction F as [| pc pieces Hpc F IH]; simpl; [eauto|].
  destruct IH as [rest R]. rewrite R.
  destruct pc as [t | name at_ | name at_ | at_ | at_]; simpl in Hpc; try discriminate; [eauto| |
    destruct (find_variable _ _); discriminate].
  destruct (find_variable name (setting_scope s)) as [info|] eqn:Fv; [|discriminate].
  destruct (lookup_matches _ _ _ _ M Fv) as [v [L T]]. rewrite L.
  destruct (variable_type info) eqn:Vt; try discriminate; destruct v; simpl in T; try discriminate; eauto.
Qed.

Lemma not_stuck_input : Input_ended <> Stuck. Proof. discriminate. Qed.
Lemma not_stuck_random : forall a b c, Random_range_not_designed a b c <> Stuck. Proof. discriminate. Qed.

Lemma apply_library_safe : forall s env which values given at_,
  matches (setting_scope s) env -> target_ready s (Library which) values given ->
  safe (outcome_ok (target_gives (Library which)) (target_can_fail (Library which)))
       (apply_library env which values given at_).
Proof.
  intros s env which values given at_ M R.
  destruct which as [[] same_line | [] | | |]; simpl in R.
  - destruct R as [v [pieces [p [G F]]]]. subst given. destruct (fill_ok _ _ _ M F) as [t Ft].
    simpl. unfold format_text. rewrite Ft. repeat constructor.
  - destruct values as [| [] [|]]; simpl in R; try discriminate. simpl. repeat constructor.
  - destruct R as [v [pieces [p [G F]]]]. subst given. destruct (fill_ok _ _ _ M F) as [t Ft].
    simpl. unfold format_text. rewrite Ft. repeat constructor.
  - destruct values as [| [] [|]]; simpl in R; destruct R as [R|R]; try discriminate; simpl; repeat constructor.
  - destruct values as [| [] [|]]; simpl in R; try discriminate. simpl.
    destruct (whole_of_text _); repeat constructor.
  - destruct values as [| [] [|]]; simpl in R; try discriminate. simpl.
    constructor. intros [answer|]; constructor; [reflexivity | apply not_stuck_input].
  - destruct values as [| [] [| [] [|]]]; simpl in R; try discriminate. simpl.
    destruct (_ <=? _)%Z; constructor; [intros; constructor; reflexivity | apply not_stuck_random].
Qed.

Lemma check_statement_setting : forall s st s' ds, check_statement s st = (s', ds) ->
  setting_program s' = setting_program s /\ setting_in_loop s' = setting_in_loop s /\
  setting_returns s' = setting_returns s /\ exists extra, setting_scope s' = extra ++ setting_scope s.
Proof.
  intros s st s' ds; destruct st; cbn [check_statement];
    repeat match goal with
    | |- context [let (_, _) := ?x in _] => destruct x
    | |- context [match ?x with _ => _ end] => destruct x eqn:?
    end; intros H; inversion H; subst; simpl; repeat split; try (exists []; reflexivity).
  all: try (unfold declare, with_scope; simpl;
    match goal with |- exists extra, ?x :: ?l = extra ++ ?l => exists [x]; reflexivity end).
  all: assumption.
Qed.

Definition clean (a : checked_argument) : Prop := argument_diagnostics a = [].

Lemma check_pairs_clean : forall procedure parameters args,
  length args = length parameters -> check_pairs procedure parameters args = [] -> Forall clean args.
Proof.
  intros procedure parameters args; revert parameters.
  induction args as [| a args IH]; intros [| [expected wanted] parameters] L C; simpl in *; try discriminate;
    constructor.
  - apply app_nil in C as [C _]. exact C.
  - apply app_nil in C as [_ C]. apply app_nil in C as [_ C]. apply app_nil in C as [_ C]. eapply IH; eauto.
Qed.

Lemma check_against_clean : forall procedure parameters args at_ declared,
  check_against procedure parameters args at_ declared = [] -> Forall clean args.
Proof.
  intros procedure parameters args at_ declared; unfold check_against.
  destruct (negb _) eqn:N; [intros C; apply app_nil in C; destruct C as [_ C]; discriminate|].
  destruct (labels_reordered _ _); [intros C; apply app_nil in C; destruct C as [_ C]; discriminate|].
  intros C. apply negb_false_iff, Nat.eqb_eq in N. eapply check_pairs_clean; eauto.
Qed.

Lemma check_single_clean : forall procedure args at_ t p,
  check_single procedure args at_ = (Some (t, p), []) -> Forall clean args.
Proof.
  intros procedure [| a [| b rest]] at_ t p; unfold check_single; simpl; intros H; try discriminate.
  destruct (argument_label a) as [[? ?]|]; [discriminate|].
  destruct (argument_type a); inversion H. constructor; [assumption | constructor].
Qed.

Lemma check_format_clean : forall s args at_, check_format s args at_ = [] -> Forall clean args.
Proof.
  intros s [| a [| b rest]] at_; unfold check_format; simpl; intros H; try discriminate; try (apply app_nil in H; destruct H; discriminate).
  destruct (argument_label a) as [[? ?]|]; [apply app_nil in H; destruct H; discriminate|].
  destruct (argument_value a); try (apply app_nil in H; destruct H; discriminate).
  apply app_nil in H as [H _]. constructor; [assumption | constructor].
Qed.

Ltac single_clean :=
  match goal with
  | |- context [check_single ?procedure ?args ?at_] =>
      destruct (check_single procedure args at_) as [[[[] ?]|] d] eqn:Sg; simpl;
      try (intros C; apply app_nil in C; destruct C as [_ C]; discriminate C);
      try (intros C; subst; eapply check_single_clean; exact Sg);
      try (intros C; subst; exfalso; refine (single_reported _ _ _ _ _ Sg _); [|reflexivity])
  end.

Lemma call_clean : forall s name args at_,
  call_diagnostics (check_call s name args at_) = [] -> Forall reported args -> Forall clean args.
Proof.
  intros s name args at_. unfold check_call.
  destruct (resolve (setting_program s) name) as [target | problems] eqn:Res; simpl.
  2:{ intros C. apply app_nil in C as [C _]. exfalso; eapply resolve_reported; eauto. }
  intros C R; revert C.
  destruct target as [which | d]; [destruct which as [[] ? | [] | | |] |]; simpl;
    try apply check_format_clean; try apply check_against_clean; single_clean; assumption.
Qed.

Lemma step_clean : forall s name slid args at_,
  call_diagnostics (check_step s name slid args at_) = [] -> Forall clean args.
Proof.
  intros s name slid args at_. unfold check_step, check_step_target.
  destruct (resolve (setting_program s) name) as [target | problems] eqn:Res; simpl.
  2:{ intros C. apply app_nil in C as [C _]. exfalso; eapply resolve_reported; eauto. }
  destruct target as [which | d]; [destruct which as [[] ? | [] | | |] |]; simpl;
    repeat match goal with
    | |- context [match ?x with _ => _ end] => destruct x
    end; simpl; intros C;
    try (apply app_nil in C; destruct C as [_ C]; discriminate C);
    try (apply app_nil in C; destruct C as [_ C]); eapply check_against_clean; eauto.
Qed.

(* ---------------------------------------------------------------------------------------------------------------- *)
(* The proof: by induction on the fuel, every part of an accepted program is safe *)

Lemma as_value_some : forall c at_ t d, as_value c at_ = (Some t, d) ->
  call_gives c = Gives_value t /\ call_can_fail c = false /\ call_diagnostics c = d.
Proof.
  intros c at_ t d; unfold as_value. destruct (call_gives c), (call_can_fail c); intros H; inversion H; auto.
Qed.

Lemma typed_some : forall s e t d, check_expression s e = (t, d) -> d = [] -> exists t', t = Some t'.
Proof.
  intros s e t d E D. destruct t as [t'|]; [eauto|].
  exfalso. apply (proj1 unknown_reported e s); rewrite E; simpl; auto.
Qed.

Lemma expect_nil : forall found expected f, expect (Some found) expected f = [] -> found = expected.
Proof. intros found expected f; unfold expect. destruct (type_eqb _ _) eqn:Q; [|discriminate]. intros _; apply type_eqb_true; auto. Qed.

Section Safety_proof.

(* The program, which the checker accepts. *)

Variable p : program.
Hypothesis accepted : check_program p = [].

Definition expression_safe (fuel : nat) := forall s e t env,
  setting_program s = p -> matches (setting_scope s) env -> check_expression s e = (Some t, []) ->
  safe (has_type t) (evaluate fuel p env e).

Definition arguments_safe (fuel : nat) := forall s given env,
  setting_program s = p -> matches (setting_scope s) env -> Forall clean (check_arguments s given) ->
  safe (typed (check_arguments s given)) (evaluate_arguments fuel p env given).

Definition apply_safe (fuel : nat) := forall s name values given at_ env target,
  setting_program s = p -> matches (setting_scope s) env -> resolve p name = Resolved target ->
  target_ready s target values given ->
  safe (outcome_ok (target_gives target) (target_can_fail target)) (apply fuel p env name values given at_).

Definition call_safe (fuel : nat) := forall s name given at_ env,
  setting_program s = p -> matches (setting_scope s) env ->
  call_diagnostics (check_call s name (check_arguments s given) at_) = [] ->
  safe (outcome_ok (call_gives (check_call s name (check_arguments s given) at_))
                   (call_can_fail (check_call s name (check_arguments s given) at_)))
       (call fuel p env name given at_).

Definition slide_safe (fuel : nat) := forall s v path env,
  setting_program s = p -> matches (setting_scope s) env -> path <> No_steps ->
  call_diagnostics (check_steps s (Some (type_of_value v)) path) = [] ->
  safe (outcome_ok (call_gives (check_steps s (Some (type_of_value v)) path))
                   (call_can_fail (check_steps s (Some (type_of_value v)) path)))
       (slide fuel p env v path).

Definition statement_safe (fuel : nat) := forall s st s' env,
  setting_program s = p -> matches (setting_scope s) env -> check_statement s st = (s', []) ->
  safe (flow_ok s s' (always_returns st)) (execute fuel p env st).

Definition statements_safe (fuel : nat) := forall s b env names,
  setting_program s = p -> matches (setting_scope s) env -> check_block s b = (names, []) ->
  safe (block_ok s (always_returns_block b)) (execute_statements fuel p env b).

Definition block_safe (fuel : nat) := forall s b env names,
  setting_program s = p -> matches (setting_scope s) env -> check_block s b = (names, []) ->
  safe (flow_ok s s (always_returns_block b)) (execute_block fuel p env b).

Definition else_safe (fuel : nat) := forall s e env names,
  setting_program s = p -> matches (setting_scope s) env -> check_else s e = (names, []) ->
  safe (flow_ok s s (always_returns_else e)) (execute_else fuel p env e).

Definition all_safe (fuel : nat) :=
  expression_safe fuel /\ arguments_safe fuel /\ apply_safe fuel /\ call_safe fuel /\ slide_safe fuel /\
  statement_safe fuel /\ statements_safe fuel /\ block_safe fuel /\ else_safe fuel.

Lemma out_of_fuel_safe : forall (A : Type) (P : A -> Prop), safe P (Stop Out_of_fuel).
Proof. intros. constructor. discriminate. Qed.

Lemma no_fuel_safe : all_safe 0.
Proof.
  unfold all_safe, expression_safe, arguments_safe, apply_safe, call_safe, slide_safe, statement_safe,
    statements_safe, block_safe, else_safe.
  repeat split; intros; apply out_of_fuel_safe.
Qed.

Lemma expression_step : forall f, all_safe f -> expression_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s e t env Ps M H.
  destruct e as [digits at_ | value pieces at_ | b at_ | name at_ | word at_ | o at_ | o at_
                | operator l r operator_at at_ | inner at_ | name given at_ | attempt fallback operator_at at_
                | start path at_ | at_]; cbn [check_expression] in H; cbn [evaluate].
  - destruct (whole_of_literal digits) as [n|]; [destruct (_ && _)|]; inversion H; subst. constructor; reflexivity.
  - inversion H; subst. constructor; reflexivity.
  - inversion H; subst. constructor; reflexivity.
  - destruct (find_variable name (setting_scope s)) as [info|] eqn:F; inversion H; subst.
    destruct (lookup_matches _ _ _ _ M F) as [v [L T]]. rewrite L. constructor. exact T.
  - destruct (is_library_word word); [inversion H|]. destruct (find_definition _ _); inversion H.
  - destruct (check_expression s o) as [[[]|] ds] eqn:Eo; inversion H; subst.
    eapply safe_bind; [eapply E; eauto|]. intros v Tv. destruct v; simpl in Tv; try discriminate.
    apply safe_checked_whole.
  - destruct (check_expression s o) as [[[]|] ds] eqn:Eo; inversion H; subst.
    eapply safe_bind; [eapply E; eauto|]. intros v Tv. apply safe_need_bool; [exact Tv | |]; constructor; reflexivity.
  - destruct (check_expression s l) as [a dl] eqn:El, (check_expression s r) as [b dr] eqn:Er.
    destruct (beside_maths operator l r operator_at); [inversion H|].
    destruct a as [a|], b as [b|]; try (inversion H; fail).
    destruct (binary_type operator a b l r operator_at) as [t' d] eqn:Bt. inversion H; subst.
    apply app_nil in H2 as [? H2]; apply app_nil in H2 as [? ?]; subst.
    destruct operator; cbn [evaluate];
      try (simpl in Bt; destruct a, b; inversion Bt; subst;
           eapply safe_bind; [eapply E; eauto|]; intros x Tx;
           apply safe_need_bool; [exact Tx| |]; try (constructor; reflexivity);
           eapply safe_bind; [eapply E; eauto|]; intros y Ty;
           apply safe_need_bool; [exact Ty| |]; constructor; reflexivity).
    all: eapply safe_bind; [eapply E; eauto|]; intros x Tx;
         eapply safe_bind; [eapply E; eauto|]; intros y Ty;
         eapply safe_apply_binary; eauto; discriminate.
  - eapply E; eauto.
  - destruct (as_value _ at_) as [t' d] eqn:Av. inversion H; subst.
    apply app_nil in H2 as [? _]; subst. apply as_value_some in Av as (G & F & D).
    eapply safe_bind; [eapply C; eauto|]. rewrite G, F. intros [v| |] O; simpl in O; try contradiction; try discriminate.
    constructor; exact O.
  - destruct (check_expression s fallback) as [ft df] eqn:Ef.
    destruct attempt as [ | | | | | | | | | name given call_at | | | ];
      try (match goal with H : (let (_, _) := ?x in _) = _ |- _ => destruct x end; inversion H; fail).
    cbn [evaluate].
    destruct (check_call s name (check_arguments s given) call_at) as [g fails diagnostics] eqn:Cc.
    destruct g, fails; simpl in H; inversion H; subst.
    apply app_nil in H2 as [D H2]; apply app_nil in H2 as [_ H2]; apply app_nil in H2 as [Df Ex].
    subst. destruct (typed_some _ _ _ _ Ef eq_refl) as [ft' Ft]. subst ft. apply expect_nil in Ex. subst ft'.
    eapply safe_bind; [eapply C; eauto; rewrite Cc; reflexivity|]. rewrite Cc. simpl.
    intros [v| |] O; simpl in O; try contradiction.
    + constructor; exact O.
    + eapply E; eauto.
  - destruct (check_expression s start) as [ts ds] eqn:Es.
    destruct (as_value (check_steps s ts path) at_) as [result dc] eqn:Av. inversion H; subst.
    apply app_nil in H2 as [? H2]; apply app_nil in H2 as [Sh ?]; subst.
    destruct (typed_some _ _ _ _ Es eq_refl) as [ts' T]. subst ts.
    apply as_value_some in Av as (G & F & D).
    assert (path <> No_steps) as N by (intros Z; subst; eapply slide_shape_empty; eauto).
    eapply safe_bind; [eapply E; eauto|]. intros v Tv. unfold has_type in Tv. subst ts'.
    eapply safe_bind; [eapply Sl; eauto|]. rewrite G, F. intros [w| |] O; simpl in O; try contradiction; try discriminate.
    constructor; exact O.
  - inversion H.
Qed.


Lemma arguments_step : forall f, all_safe f -> arguments_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s given env Ps M K.
  destruct given as [| label value rest]; cbn [evaluate_arguments check_arguments] in *.
  - constructor. constructor.
  - destruct (check_expression s value) as [t d] eqn:Ev. inversion K as [| ? ? Kd Kr]; subst.
    unfold clean in Kd; simpl in Kd; subst d.
    destruct (typed_some _ _ _ _ Ev eq_refl) as [t' T]; subst t.
    eapply safe_bind; [eapply E; eauto|]. intros v Tv.
    eapply safe_bind; [eapply A; eauto|]. intros vs Tvs. constructor. constructor; simpl; [|exact Tvs].
    rewrite Tv; reflexivity.
Qed.

Lemma call_step : forall f, all_safe f -> call_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s name given at_ env Ps M D.
  cbn [call].
  assert (Forall clean (check_arguments s given)) as K.
  { eapply call_clean; eauto. apply (proj1 (proj2 unknown_reported)). }
  eapply safe_bind; [eapply A; eauto|]. intros values T.
  destruct (call_ready _ _ _ _ _ D T) as (target & R & Ready & G & F).
  rewrite G, F. rewrite Ps in R. eapply Ap; eauto.
Qed.

Lemma slide_step : forall f, all_safe f -> slide_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s v path env Ps M N D.
  destruct path as [| name bracketed given at_ rest]; [contradiction N; reflexivity|].
  cbn [slide check_steps] in *.
  set (this := check_step s name (Some (type_of_value v)) (check_arguments s given) at_) in *.
  assert (call_diagnostics this = [] ->
            safe (typed (check_arguments s given)) (evaluate_arguments f p env given)) as Av.
  { intros Dc. eapply A; eauto. eapply step_clean; eauto. }
  assert (call_diagnostics this = [] -> forall values, typed (check_arguments s given) values ->
            safe (outcome_ok (call_gives this) (call_can_fail this)) (apply f p env name (v :: values) given at_)) as Rv.
  { intros Dc values T. destruct (step_ready _ _ _ _ _ _ Dc T) as (target & R & Rd & G & F).
    unfold this. rewrite G, F. rewrite Ps in R. eapply Ap; eauto. }
  destruct rest as [| name2 bracketed2 given2 at2 rest2].
  - eapply safe_bind; [exact (Av D)|]. intros values T.
    eapply safe_bind; [exact (Rv D values T)|]. intros o O. constructor. exact O.
  - destruct (as_value this at_) as [next d] eqn:Avl. simpl in D. apply app_nil in D as [Dd Dl]. subst d.
    destruct next as [tn|].
    2:{ exfalso. refine (as_value_reported this at_ _ _ _); [| rewrite Avl; reflexivity | rewrite Avl; reflexivity].
        intros G. apply step_reported; [| exact G]. apply (proj1 (proj2 unknown_reported)). }
    apply as_value_some in Avl as (G & F & Dt).
    eapply safe_bind; [exact (Av Dt)|]. intros values T.
    eapply safe_bind; [exact (Rv Dt values T)|]. rewrite G, F. intros [w| |] O; simpl in O; try contradiction; try discriminate.
    subst tn. eapply Sl; eauto. discriminate.
Qed.


Definition body_ok (d : definition) (fl : flow) : Prop :=
  match fl with
  | Continue _ => definition_returns d = None
  | Leave_loop _ => False
  | Leave_procedure r => result_ok (definition_returns d) r
  end.

Lemma procedure_result_safe : forall d fl, body_ok d fl -> safe (outcome_ok (gives_of d) false) (procedure_result d fl).
Proof.
  intros d [env|env|[v|]] H; unfold procedure_result, gives_of; simpl in H;
    destruct (definition_returns d); simpl in *; try contradiction; try discriminate; repeat constructor; auto.
Qed.

Definition start_setting (d : definition) : setting :=
  {| setting_program := p; setting_scope := rev (map parameter_info (definition_parameters d));
     setting_ended := []; setting_in_loop := false; setting_procedure := procedure_label d;
     setting_returns := definition_returns d |}.

Lemma definition_accepted : forall d, In d p -> check_definition p d = [].
Proof.
  intros d D. pose proof accepted as Acc. unfold check_program in Acc. apply app_nil in Acc as [_ F].
  eapply flat_map_nil; eauto.
Qed.

Lemma definition_parts : forall d, check_definition p d = [] ->
  exists rest names,
    (definition_body d = rest \/ exists pairs at_, definition_body d = Then (Labels pairs at_) rest) /\
    check_block (start_setting d) rest = (names, []) /\
    (definition_returns d <> None -> always_returns_block rest = true) /\
    (definition_name d = Main -> definition_parameters d = [] /\ definition_returns d = None).
Proof.
  intros d H. unfold check_definition in H. cbn zeta in H.
  destruct (labels_and_rest (definition_body d)) as [labels_line rest] eqn:LR.
  apply app_nil in H as [_ H]. apply app_nil in H as [Mr H]. apply app_nil in H as [Mp H].
  apply app_nil in H as [_ H]. apply app_nil in H as [Cb H]. apply app_nil in H as [_ Mret].
  match type of Cb with snd (check_block ?st rest) = [] => change st with (start_setting d) in Cb end.
  destruct (check_block (start_setting d) rest) as [names dr] eqn:Cbk. simpl in Cb. subst dr.
  exists rest, names. repeat split.
  - unfold labels_and_rest in LR. destruct (definition_body d) as [|first more]; [inversion LR; auto|].
    destruct first; inversion LR; subst; auto. right; eauto.
  - exact Cbk.
  - intros R. destruct (definition_returns d); [|contradiction].
    destruct (always_returns_block rest); [reflexivity | discriminate].
  - match goal with K : definition_name d = Main |- _ => rename K into Mn end. rewrite Mn in Mp. destruct (definition_parameters d); [reflexivity | discriminate].
  - match goal with K : definition_name d = Main |- _ => rename K into Mn end. rewrite Mn in Mr. destruct (definition_returns d); [discriminate | reflexivity].
Qed.

Lemma restore_body_ok : forall d a env fl,
  (definition_returns d <> None -> a = true) ->
  (match fl with
   | Continue _ => a = false
   | Leave_loop _ => setting_in_loop (start_setting d) = true
   | Leave_procedure r => result_ok (definition_returns d) r
   end) ->
  body_ok d (restore_flow env fl).
Proof.
  intros d a env [e|e|r] R H; simpl in *; auto; [|discriminate].
  destruct (definition_returns d); [|reflexivity]. subst a. specialize (R ltac:(discriminate)). discriminate.
Qed.

Definition safe_below (f : nat) := forall m, m <= f -> all_safe m.

Lemma body_safe : forall f d locals, safe_below f -> In d p ->
  matches (setting_scope (start_setting d)) locals ->
  safe (body_ok d) (execute_block f p locals (definition_body d)).
Proof.
  intros f d locals Below D M.
  destruct (definition_parts d (definition_accepted d D)) as (rest & names & Body & Cb & Always & _).
  destruct Body as [Body | (pairs & at_ & Body)]; rewrite Body.
  - destruct (Below f (le_n f)) as (_ & _ & _ & _ & _ & _ & _ & B & _).
    eapply safe_weaken; [eapply (B (start_setting d)); eauto; reflexivity|].
    intros fl Fl. destruct fl as [e|e|r]; simpl in *; [| destruct Fl; discriminate | exact Fl].
    destruct Fl as [A _]. destruct (definition_returns d) eqn:Rt; [|reflexivity].
    rewrite (Always ltac:(discriminate)) in A. discriminate.
  - destruct f as [|[|[|f3]]]; cbn [execute_block execute_statements execute bind];
      try apply out_of_fuel_safe.
    destruct (Below (S f3) ltac:(lia)) as (_ & _ & _ & _ & _ & _ & Ss & _ & _).
    eapply safe_bind; [eapply (Ss (start_setting d)); eauto; reflexivity|].
    intros fl Fl. constructor. eapply restore_body_ok; [exact Always|].
    destruct fl as [e|e|r]; simpl in *; [destruct Fl; auto | destruct Fl; auto | exact Fl].
Qed.

Lemma apply_step : forall f, safe_below f -> apply_safe (S f).
Proof.
  intros f Below s name values given at_ env target Ps M R Ready.
  cbn [apply]. rewrite R.
  destruct target as [which | d].
  - apply (apply_library_safe s); [exact M | exact Ready].
  - simpl in Ready.
    assert (Forall2 (fun p v => parameter_type p = type_of_value v) (definition_parameters d) values) as F2.
    { clear - Ready. revert values Ready. induction (definition_parameters d) as [|q qs IH];
        intros [|v vs] Ready; simpl in Ready; try discriminate; constructor.
      - inversion Ready; auto.
      - apply IH. inversion Ready; auto. }
    destruct (parameters_match _ _ F2) as [locals [Bp Ml]]. rewrite Bp.
    eapply safe_bind; [eapply body_safe; eauto; eapply resolve_user_in; eauto|].
    intros fl Fl. simpl. apply procedure_result_safe. exact Fl.
Qed.


Lemma run_call_nothing : forall c procedure at_,
  check_run_call c procedure at_ = [] -> (call_gives c = Gives_unknown -> call_diagnostics c <> []) ->
  call_gives c = Gives_nothing /\ call_can_fail c = false /\ call_diagnostics c = [].
Proof.
  intros c procedure at_; unfold check_run_call.
  destruct (call_gives c), (call_can_fail c); intros H U;
    try (apply app_nil in H as [_ H]; discriminate H); auto.
  exfalso; apply U; auto.
Qed.

Lemma with_ended_scope : forall s e, setting_scope (with_ended s e) = setting_scope s.
Proof. reflexivity. Qed.

Lemma statement_step : forall f, all_safe f -> statement_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s st s' env Ps M H.
  destruct st as [changeable name name_at keyword_at written value at_ | name name_at value operator_at at_
                 | action at_ | condition then_part rest at_ | condition body at_ | at_ | [value|] at_ | pairs at_];
    cbn [check_statement] in H; cbn [execute always_returns].
  - (* let and var *)
    destruct (check_expression s value) as [t ds] eqn:Ev. inversion H; subst s'.
    apply app_nil in H2 as [_ H2]. apply app_nil in H2 as [Ds Mm]. subst ds.
    destruct (typed_some _ _ _ _ Ev eq_refl) as [t' T]; subst t. apply expect_nil in Mm; subst t'.
    eapply safe_bind; [eapply E; eauto|]. intros v Tv. constructor. simpl. split; [reflexivity|].
    unfold declare, with_scope; simpl. constructor; [simpl; split; auto|exact M].
  - (* = *)
    destruct (check_expression s value) as [t ds] eqn:Ev.
    destruct (find_variable name (setting_scope s)) as [info|] eqn:Fv; [|inversion H].
    destruct (variable_kind info); inversion H; subst s'.
    apply app_nil in H2 as [Ds Mm]. subst ds.
    destruct (typed_some _ _ _ _ Ev eq_refl) as [t' T]; subst t. apply expect_nil in Mm.
    eapply safe_bind; [eapply E; eauto|]. intros v Tv.
    destruct (assign_matches _ _ _ _ v M Fv) as [env' [As Ma]]; [unfold has_type in Tv; congruence|].
    rewrite As. constructor. simpl. auto.
  - (* a call or slide standing alone *)
    inversion H; subst s'. unfold check_run in H2.
    destruct action as [ | | | | | | | | | name given call_at | attempt fallback operator_at fat | start path sat | ];
      try (apply app_nil in H2 as [_ H2]; discriminate H2); try discriminate H2.
    + apply app_nil in H2 as [Rc _].
      apply run_call_nothing in Rc as (G & F & D).
      2:{ apply call_reported. apply (proj1 (proj2 unknown_reported)). }
      eapply safe_bind; [eapply C; eauto|]. rewrite G, F. intros [v| |] O; simpl in O; try contradiction.
      constructor. simpl. auto.
    + destruct attempt; try (apply app_nil in H2 as [_ H2]; discriminate H2).
      destruct (check_expression _ _) as [[t|] ds] eqn:Ef; [apply app_nil in H2 as [_ H2]; discriminate H2|].
      exfalso. match type of Ef with check_expression _ ?e = _ =>
        apply (proj1 unknown_reported e s); rewrite Ef; simpl; auto end.
    + destruct (check_expression s start) as [ts ds] eqn:Es.
      apply app_nil in H2 as [Ds H2]. apply app_nil in H2 as [Sh Rc]. subst ds.
      destruct (typed_some _ _ _ _ Es eq_refl) as [ts' T]; subst ts.
      assert (path <> No_steps) as N by (intros Z; subst; eapply slide_shape_empty; eauto).
      apply run_call_nothing in Rc as (G & F & D).
      2:{ intros U. apply (proj2 (proj2 unknown_reported)); auto. }
      eapply safe_bind; [eapply E; eauto|]. intros v Tv. unfold has_type in Tv. subst ts'.
      eapply safe_bind; [eapply Sl; eauto|]. rewrite G, F. intros [w| |] O; simpl in O; try contradiction.
      constructor. simpl. auto.
  - (* if *)
    destruct (check_block s then_part) as [ended_then dt] eqn:Bt.
    destruct (check_else s rest) as [ended_else de] eqn:Be.
    inversion H; subst s'.
    apply app_nil in H2 as [Dc H2]. apply app_nil in H2 as [Dt De]. subst dt de.
    unfold check_condition in Dc. destruct (check_expression s condition) as [tc dc] eqn:Ec.
    destruct tc as [[]|]; try (apply app_nil in Dc as [_ Dc]; discriminate Dc);
      [| exfalso; apply (proj1 unknown_reported condition s); rewrite Ec; simpl; auto].
    subst dc.
    eapply safe_bind; [eapply E; eauto|]. intros c Tc. apply safe_need_bool; [exact Tc| |].
    + eapply safe_weaken; [eapply B; eauto|]. intros [e|e|r] Fl; simpl in *; auto.
      destruct Fl as [Fa Fm]; rewrite Fa; auto.
    + eapply safe_weaken; [eapply X; eauto|]. intros [e|e|r] Fl; simpl in *; auto.
      destruct Fl as [Fa Fm]; rewrite Fa, andb_false_r; auto.
  - (* while *)
    destruct (check_block (in_loop s) body) as [ended_body db] eqn:Bb.
    inversion H; subst s'.
    apply app_nil in H2 as [Dc Db]. subst db.
    unfold check_condition in Dc. destruct (check_expression s condition) as [tc dc] eqn:Ec.
    destruct tc as [[]|]; try (apply app_nil in Dc as [_ Dc]; discriminate Dc);
      [| exfalso; apply (proj1 unknown_reported condition s); rewrite Ec; simpl; auto].
    subst dc.
    eapply safe_bind; [eapply E; eauto|]. intros c Tc. apply safe_need_bool; [exact Tc| |].
    + eapply safe_bind; [eapply (B (in_loop s)); eauto|]. intros [e|e|r] Fl; simpl in Fl.
      * destruct Fl as [_ Fm]. apply (St s (While condition body at_) _ e Ps Fm). cbn [check_statement].
        rewrite Bb. unfold check_condition. rewrite Ec. reflexivity.
      * constructor. simpl. destruct Fl as [_ Fm]. auto.
      * constructor. exact Fl.
    + constructor. simpl. auto.
  - (* break *)
    destruct (setting_in_loop s) eqn:L; inversion H; subst. constructor. simpl. auto.
  - (* return with a value *)
    destruct (check_expression s value) as [t ds] eqn:Ev.
    destruct (setting_returns s) as [expected|] eqn:Rt; inversion H; subst s'.
    + apply app_nil in H2 as [Ds Mm]. subst ds.
      destruct (typed_some _ _ _ _ Ev eq_refl) as [t' T]; subst t. apply expect_nil in Mm; subst t'.
      eapply safe_bind; [eapply E; eauto|]. intros v Tv. constructor. simpl. rewrite Rt. exact Tv.
    + apply app_nil in H2 as [_ H2]. discriminate H2.
  - (* bare return *)
    destruct (setting_returns s) eqn:Rt; inversion H; subst. constructor. simpl. rewrite Rt. exact I.
  - inversion H.
Qed.


Lemma statements_step : forall f, all_safe f -> statements_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s b env names Ps M H.
  destruct b as [| st rest]; cbn [check_block] in H; cbn [execute_statements].
  - inversion H; subst. constructor. simpl. split; [reflexivity|]. exists []; exact M.
  - destruct (check_statement s st) as [s' ds] eqn:Cs.
    destruct (check_block s' rest) as [names' dr] eqn:Cr.
    inversion H; subst. apply app_nil in H2 as [Ds H2]. apply app_nil in H2 as [_ Dr]. subst ds dr.
    destruct (check_statement_setting _ _ _ _ Cs) as (Pp & Pl & Pr & extra & Sc).
    eapply safe_bind; [eapply St; eauto|]. intros fl Fl.
    destruct fl as [e|e|r]; simpl in Fl.
    + destruct Fl as [Fa Fm].
      eapply safe_weaken; [eapply (Ss s' rest e names'); eauto; congruence|].
      intros [e'|e'|r'] Bk; simpl in *; rewrite ?Pl, ?Pr in *.
      * destruct Bk as [Ba [extra' Bm]]. split.
        -- destruct rest; [simpl in *; congruence | exact Ba].
        -- exists (extra' ++ extra). rewrite <- app_assoc, <- Sc. exact Bm.
      * destruct Bk as [Ba [extra' Bm]]. split; [exact Ba|].
        exists (extra' ++ extra). rewrite <- app_assoc, <- Sc. exact Bm.
      * exact Bk.
    + constructor. simpl. destruct Fl as [Fa Fm]. split; [exact Fa | exists []; exact Fm].
    + constructor. exact Fl.
Qed.

Lemma block_step : forall f, all_safe f -> block_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s b env names Ps M H.
  cbn [execute_block]. eapply safe_bind; [eapply Ss; eauto|]. intros [e|e|r] Bk; constructor; simpl in *.
  - destruct Bk as [Ba [extra Bm]]. split; [exact Ba|]. eapply restore_matches; eauto.
  - destruct Bk as [Ba [extra Bm]]. split; [exact Ba|]. eapply restore_matches; eauto.
  - exact Bk.
Qed.

Lemma else_step : forall f, all_safe f -> else_safe (S f).
Proof.
  intros f (E & A & Ap & C & Sl & St & Ss & B & X) s e env names Ps M H.
  destruct e as [| body at_ | condition body rest at_]; cbn [check_else] in H; cbn [execute_else always_returns_else].
  - constructor. simpl. auto.
  - eapply B; eauto.
  - destruct (check_block s body) as [nb db] eqn:Cb. destruct (check_else s rest) as [nr dr] eqn:Cr.
    inversion H; subst. apply app_nil in H2 as [Dc H2]. apply app_nil in H2 as [Db Dr]. subst db dr.
    unfold check_condition in Dc. destruct (check_expression s condition) as [tc dc] eqn:Ec.
    destruct tc as [[]|]; try (apply app_nil in Dc as [_ Dc]; discriminate Dc);
      [| exfalso; apply (proj1 unknown_reported condition s); rewrite Ec; simpl; auto].
    subst dc.
    eapply safe_bind; [eapply E; eauto|]. intros c Tc. apply safe_need_bool; [exact Tc| |].
    + eapply safe_weaken; [eapply B; eauto|]. intros [e|e|r] Fl; simpl in *; auto.
      destruct Fl as [Fa Fm]; rewrite Fa; auto.
    + eapply safe_weaken; [eapply X; eauto|]. intros [e|e|r] Fl; simpl in *; auto.
      destruct Fl as [Fa Fm]; rewrite Fa, andb_false_r; auto.
Qed.

Lemma every_fuel_safe : forall fuel, all_safe fuel.
Proof.
  intros fuel. induction fuel as [fuel IH] using (well_founded_induction lt_wf).
  destruct fuel as [|f]; [apply no_fuel_safe|].
  assert (all_safe f) as Af by (apply IH; lia).
  assert (safe_below f) as Below by (intros m Le; apply IH; lia).
  repeat split.
  - apply expression_step; exact Af.
  - apply arguments_step; exact Af.
  - apply apply_step; exact Below.
  - apply call_step; exact Af.
  - apply slide_step; exact Af.
  - apply statement_step; exact Af.
  - apply statements_step; exact Af.
  - apply block_step; exact Af.
  - apply else_step; exact Af.
Qed.

Lemma main_exists : forall mains names q, check_names mains names q = [] ->
  mains <> [] \/ exists d, find_main q = Some d.
Proof.
  intros mains names q; revert mains names. induction q as [|d rest IH]; intros mains names H; simpl in H.
  - destruct mains; [discriminate | left; discriminate].
  - unfold find_main; simpl. destruct (definition_name d) eqn:N; simpl.
    + right. exists d. unfold is_main. rewrite N. reflexivity.
    + apply app_nil in H as [_ H]. apply app_nil in H as [_ H].
      destruct (IH _ _ H) as [Ne | [d' F]]; [left; exact Ne|].
      right. exists d'. unfold is_main at 1. rewrite N. exact F.
Qed.

Theorem run_safe : forall fuel, safe (fun _ => True) (run fuel p).
Proof.
  intros fuel. unfold run.
  destruct (main_exists [] [] p) as [Ne | [d F]].
  { pose proof accepted as Acc. unfold check_program in Acc. apply app_nil in Acc as [Cn _]. exact Cn. }
  { contradiction Ne; reflexivity. }
  rewrite F.
  assert (In d p) as D by (apply find_some in F; tauto).
  assert (definition_name d = Main) as Mn.
  { apply find_some in F as [_ Im]. unfold is_main in Im. destruct (definition_name d); [reflexivity|discriminate]. }
  destruct (definition_parts d (definition_accepted d D)) as (_ & _ & _ & _ & _ & Main_ok).
  destruct (Main_ok Mn) as [Np Nr].
  eapply safe_bind.
  - apply (body_safe fuel d []); [intros m _; apply every_fuel_safe | exact D |].
    simpl. rewrite Np. constructor.
  - intros [e|e|[r|]] Bk; simpl in Bk; rewrite ?Nr in Bk; try contradiction; constructor; auto.
Qed.

End Safety_proof.

(* THEOREM: type safety.
   In plain words: if the checker accepts a program, then running it never gets stuck. However the input goes and
   whatever random numbers come back, every way through it ends in one of four ways: it reaches the end of
   `proc.main`; it stops with a failed runtime check, like D46's overflow check; it stops because something isn't
   decided yet, like `random` with `from:` above `to:`; or `ask` finds no more input. Or it's still running when the
   fuel, the evaluator's step counter, runs out. It never meets a variable that isn't there, a value of the wrong
   type, a procedure that doesn't exist, a `break` outside a loop, or a procedure with `returns` that finishes
   without giving a value.
   What it doesn't say: that the evaluator's rules are the standard's. It promises the evaluator never reaches its own
   [Stuck] cases; the tests, and later the comparison with `Unlo\n`, check that what it does is what the standard
   says. *)
Theorem type_safety : forall p fuel, check_program p = [] -> safe (fun _ => True) (run fuel p).
Proof. intros p fuel Accepted. apply run_safe. exact Accepted. Qed.
