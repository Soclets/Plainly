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

Lemma safe_need_bool : forall (A : Type) (P : A -> Prop) v a b,
  type_of_value v = Bool -> safe P a -> safe P b -> safe P (need_bool v a b).
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
  | Library (Convert_to_text false) => map type_of_value values = [Whole]
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
      destruct (argument_type a) eqn:Ta; inversion Sg; subst; eapply typed_one; eauto
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
    try (apply app_nil in C as [_ C]). erewrite check_against_typed by eauto. simpl. repeat split; congruence.
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
  - destruct values as [| [] [|]]; simpl in R; try discriminate. simpl. repeat constructor.
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
