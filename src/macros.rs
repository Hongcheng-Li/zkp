// -*- coding: utf-8; mode: rust; -*-
//
// To the extent possible under law, the authors have waived all
// copyright and related or neighboring rights to zkp,
// using the Creative Commons "CC0" public domain dedication.  See
// <http://creativecommons.org/publicdomain/zero/1.0/> for full
// details.
//
// Authors:
// - Henry de Valence <hdevalence@hdevalence.ca>

#[doc(hidden)]
#[macro_export]
macro_rules! __compute_formula_constraint {
    // Unbracket a statement
    (($public_vars:ident, $secret_vars:ident) ($($x:tt)*)) => {
        // Add a trailing +
        __compute_formula_constraint!(($public_vars,$secret_vars) $($x)* +)
    };
    // Inner part of the formula: give a list of &Scalars
    // Since there's a trailing +, we can just generate the list as normal...
    (($public_vars:ident, $secret_vars:ident)
     $( $scalar:ident * $point:ident +)+ ) => {
        vec![ $( ($secret_vars.$scalar , $public_vars.$point) )* ]
    };
}

/// Creates a module with code required to produce a non-interactive
/// zero-knowledge proof statement, to serialize it to wire format, to
/// parse from wire format, and to verify the proof statement.
///
/// The statement is specified in an embedded DSL resembling
/// Camenisch-Stadler notation.  For instance, a proof of knowledge of
/// two equal discrete logarithms ("DLEQ") is specified as:
///
/// ```rust,ignore
/// define_proof!{dleq, (x), (A, B, G, H) : A = (G * x), B = (H * x) }
/// ```
///
/// This creates a module `dleq` with code for proving knowledge of a
/// secret `x: Scalar` such that `A = G*x`, `B = H*x` for public
/// parameters `A, B, G, H: RistrettoPoint`.  In general the syntax is
///
/// ```rust,ignore
/// define_proof!{
///     module_name, // used to label proof statements
///     (x,y,z,...), // secret variable names
///     (A,B,C,...)  // public parameter names
///     :
///     LHS = (A * x + B * y + C * z + ... ),  // comma-seperated statements
///     ...
/// }
/// ```
///
/// Statements have the form `LHS = (A * x + B * y + C * z + ... )`,
/// where `LHS` is one of the points listed as a public parameter, and
/// the right-hand side is a sum of public points multiplied by secret
/// scalars.
///
/// Inside the generated module `module_name`, the macro defines three
/// structs:
///
/// A `Publics` struct corresponding to the public parameters, of the
/// form
///
/// ```rust,ignore
/// pub struct Publics<'a> { pub A: &'a RistrettoPoint, ... }
/// ```
///
/// A `Secrets` struct corresponding to the secret parameters, of the
/// form
///
/// ```rust,ignore
/// pub struct Secrets<'a> { pub x: &'a Scalar, ... }
/// ```
///
/// A `Proof` struct, of the form
///
/// ```rust,ignore
/// #[derive(Serialize, Deserialize)]
/// pub struct Proof { ... }
///
/// impl Proof {
///     pub fn create(
///         transcript: &mut Transcript,
///         publics: Publics,
///         secrets: Secrets,
///     ) -> Proof { ... }
///
///     pub fn verify(
///         &self,
///         &mut Transcript,
///         publics: Publics,
///     ) -> Result<(),()> { ... }
/// }
/// ```
///
/// The `Proof` struct derives the Serde traits, so it can be
/// serialized and deserialized to various wire formats.
///
/// The `Publics` and `Secrets` structs are used to fake named
/// arguments in the input to `create` and `verify`.  Proof creation
/// is done in constant time.  Proof verification uses variable-time
/// code.
///
/// As an example, we can create and verify a DLEQ proof as follows:
///
/// XXX readd example once API is finished.
#[macro_export]
macro_rules! define_proof {
    (
        $proof_module_name:ident // Name of the module to create
        ,
        ( $($secret_var:ident),+ ) // Secret variables, sep by commas
        ,
        ( $($instance_var:ident),* ) // Public instance variables, separated by commas
        ,
        ( $($common_var:ident),* ) // Public common variables, separated by commas
        :
        // List of statements to prove
        // Format: LHS = ( ... RHS expr ... ),
        $($lhs:ident = $statement:tt),+
    ) => {
        /// An auto-generated Schnorr proof implementation.
        pub mod $proof_module_name {
            use $crate::curve25519_dalek::scalar::Scalar;
            use $crate::curve25519_dalek::ristretto::RistrettoPoint;
            use $crate::curve25519_dalek::traits::{MultiscalarMul, VartimeMultiscalarMul};
            use $crate::rand::thread_rng;

            pub use $crate::merlin::Transcript;

            use $crate::prover::Prover;
            use $crate::verifier::Verifier;

            pub use $crate::{CompactProof, BatchableProof};

            /// The generated [`internal`] module contains lower-level
            /// functions at the level of the Schnorr constraint
            /// system API.
            pub mod internal {
                use $crate::SchnorrCS;

                /// The proof label committed to the transcript as a domain separator.
                pub const PROOF_LABEL: &'static str = stringify!($proof_module_name);

                /// A container type that holds transcript labels for secret variables.
                pub struct TranscriptLabels {
                    $( pub $secret_var: &'static str, )+
                    $( pub $instance_var: &'static str, )+
                    $( pub $common_var: &'static str, )+
                }

                /// The transcript labels used for each secret variable.
                pub const TRANSCRIPT_LABELS: TranscriptLabels = TranscriptLabels {
                    $( $secret_var: stringify!($secret_var), )+
                    $( $instance_var: stringify!($instance_var), )+
                    $( $common_var: stringify!($common_var), )+
                };

                /// A container type that simulates named parameters for [`proof_statement`].
                #[derive(Copy, Clone)]
                pub struct SecretVars<CS: SchnorrCS> {
                    $( pub $secret_var: CS::ScalarVar, )+
                }

                /// A container type that simulates named parameters for [`proof_statement`].
                #[derive(Copy, Clone)]
                pub struct PublicVars<CS: SchnorrCS> {
                    $( pub $instance_var: CS::PointVar, )+
                    $( pub $common_var: CS::PointVar, )+
                }

                /// The underlying proof statement generated by the macro invocation.
                ///
                /// This function exists separately from the proving
                /// and verification functions to allow composition of
                /// different proof statements with common variable
                /// assignments.
                pub fn proof_statement<CS: SchnorrCS>(
                    cs: &mut CS,
                    secrets: SecretVars<CS>,
                    publics: PublicVars<CS>,
                ) {
                    $(
                        cs.constrain(
                            publics.$lhs,
                            __compute_formula_constraint!( (publics, secrets) $statement ),
                        );
                    )+
                }
            }

            /// A container type that simulates passing secret variable assignments as named parameters.
            #[derive(Copy, Clone)]
            pub struct SecretAssignments<'a> {$(pub $secret_var : &'a Scalar,)+}

            /// A container type that simulates passing instance variable assignments as named parameters.
            #[derive(Copy, Clone)]
            pub struct InstanceAssignments<'a> {$(pub $instance_var : &'a RistrettoPoint,)+}

            /// A container type that simulates passing common variable assignments as named parameters.
            #[derive(Copy, Clone)]
            pub struct CommonAssignments<'a> {$(pub $common_var : &'a RistrettoPoint,)+}

            fn build_prover<'a>(
                transcript: &'a mut Transcript,
                secret_assignments: SecretAssignments,
                instance_assignments: InstanceAssignments,
                common_assignments: CommonAssignments,
            ) -> Prover<'a> {
                use self::internal::*;
                use $crate::prover::*;

                let mut prover = Prover::new(PROOF_LABEL.as_bytes(), transcript);

                let secret_vars = SecretVars {
                    $($secret_var: prover.allocate_scalar(TRANSCRIPT_LABELS.$secret_var.as_bytes(), *secret_assignments.$secret_var),)+
                };

                // XXX return compressed points
                let public_vars = PublicVars {
                    $($instance_var: prover.allocate_point(TRANSCRIPT_LABELS.$instance_var.as_bytes(), *instance_assignments.$instance_var).0,)+
                    $($common_var: prover.allocate_point(TRANSCRIPT_LABELS.$common_var.as_bytes(), *common_assignments.$common_var).0,)+
                };

                proof_statement(&mut prover, secret_vars, public_vars);

                prover
            }

            /// Given a transcript and assignments to secret and public variables, produce a proof in compact format.
            pub fn prove_compact(
                transcript: &mut Transcript,
                secret_assignments: SecretAssignments,
                instance_assignments: InstanceAssignments,
                common_assignments: CommonAssignments,
            ) -> CompactProof {
                let prover = build_prover(transcript, secret_assignments, instance_assignments, common_assignments);

                prover.prove_compact()
            }

            /// Given a transcript and assignments to secret and public variables, produce a proof in batchable format.
            pub fn prove_batchable(
                transcript: &mut Transcript,
                secret_assignments: SecretAssignments,
                instance_assignments: InstanceAssignments,
                common_assignments: CommonAssignments,
            ) -> CompactProof {
                let prover = build_prover(transcript, secret_assignments, instance_assignments, common_assignments);

                prover.prove_compact()
            }

            fn build_verifier<'a>(
                transcript: &'a mut Transcript,
                instance_assignments: InstanceAssignments,
                common_assignments: CommonAssignments,
            ) -> Verifier<'a> {
                use self::internal::*;
                use $crate::verifier::*;

                let mut verifier = Verifier::new(PROOF_LABEL.as_bytes(), transcript);

                let secret_vars = SecretVars {
                    $($secret_var: verifier.allocate_scalar(TRANSCRIPT_LABELS.$secret_var.as_bytes()),)+
                };

                // XXX take compressed points
                let public_vars = PublicVars {
                    $($instance_var: verifier.allocate_point(TRANSCRIPT_LABELS.$instance_var.as_bytes(), instance_assignments.$instance_var.compress()),)+
                    $($common_var: verifier.allocate_point(TRANSCRIPT_LABELS.$common_var.as_bytes(), common_assignments.$common_var.compress()),)+
                };

                proof_statement(&mut verifier, secret_vars, public_vars);

                verifier
            }

            /// Given a transcript and assignments to public variables, verify a proof in compact format.
            pub fn verify_compact(
                proof: &CompactProof,
                transcript: &mut Transcript,
                instance_assignments: InstanceAssignments,
                common_assignments: CommonAssignments,
            ) -> Result<(), ()> {
                let verifier = build_verifier(transcript, instance_assignments, common_assignments);

                verifier.verify_compact(proof)
            }

            /// Given a transcript and assignments to public variables, verify a proof in batchable format.
            pub fn verify_batchable(
                proof: &BatchableProof,
                transcript: &mut Transcript,
                instance_assignments: InstanceAssignments,
                common_assignments: CommonAssignments,
            ) -> Result<(), ()> {
                let verifier = build_verifier(transcript, instance_assignments, common_assignments);

                verifier.verify_batchable(proof)
            }
        }
    }
}
