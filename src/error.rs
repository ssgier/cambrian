use nix::unistd::Pid;
use std::ffi::OsString;
use std::io;
use std::process::Output;
use thiserror;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid yaml")]
    InvalidYaml(#[from] serde_yaml::Error),
    #[error("invalid json")]
    InvalidJson(#[from] serde_json::Error),
    #[error("at path {path_hint:?}: yaml must be a map")]
    YamlMustBeMap { path_hint: String },
    #[error("at path {path_hint:?}: cannot parse attribute \"{attribute_name:?}\". Number probably too big")]
    UnsignedIntConversionFailed {
        path_hint: String,
        attribute_name: String,
    },
    #[error("at path {path_hint:?}: attribute value of \"{attribute_name:?}\" must be {expected_type_hint:?}. Hint: Did you leave a trailing comma?")]
    InvalidAttributeValueType {
        path_hint: String,
        attribute_name: String,
        expected_type_hint: String,
    },
    #[error(
        "at path {path_hint:?}: attribute key \"{formatted_attribute_key:?}\" must be a string"
    )]
    InvalidAttributeKeyType {
        path_hint: String,
        formatted_attribute_key: String,
    },
    #[error("at path {path_hint:?}: unknown type name: {unknown_type_name:?}")]
    UnknownTypeName {
        path_hint: String,
        unknown_type_name: String,
    },
    #[error("at path {path_hint:?}: initial value is not within bounds provided")]
    InitNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?}: initial size is not within bounds provided")]
    InitSizeNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?}: min must be lower than max")]
    InvalidBounds { path_hint: String },
    #[error("at path {path_hint:?}: min size must be lower than max size")]
    InvalidSizeBounds { path_hint: String },
    #[error("at path {path_hint:?}: max size must not be zero")]
    ZeroMaxSize { path_hint: String },
    #[error("at path {path_hint:?}: mandatory attribute missing: {missing_attribute_name:?}")]
    MandatoryAttributeMissing {
        path_hint: String,
        missing_attribute_name: String,
    },
    #[error("at path {path_hint:?}: unexpected attribute: {unexpected_attribute_name:?}")]
    UnexpectedAttribute {
        path_hint: String,
        unexpected_attribute_name: String,
    },
    #[error("at path {path_hint:?}: sub must not be empty")]
    EmptySub { path_hint: String },
    #[error("at path {path_hint:?}: variant must at least two variant values")]
    NotEnoughVariantValues { path_hint: String },
    #[error("at path {path_hint:?}: enum must at least two enum values")]
    NotEnoughEnumValues { path_hint: String },
    #[error("at path {path_hint:?}: missing mandatory value")]
    MandatoryValueMissing { path_hint: String },
    #[error("at path {path_hint:?}: failed to convert number")]
    NumberConversionFailed { path_hint: String },
    #[error("at path {path_hint:?}: wrong type. Should be {type_hint:?}")]
    WrongTypeForValue {
        path_hint: String,
        type_hint: String,
    },
    #[error("at path {path_hint:?}: unexpected key \"{key:?}\"")]
    UnexpectedKey { path_hint: String, key: String },
    #[error("at path {path_hint:?}: unknown variant \"{variant_name:?}\"")]
    UnknownVariant {
        path_hint: String,
        variant_name: String,
    },
    #[error("at path {path_hint:?}: unknown value \"{value:?}\"")]
    UnknownValue { path_hint: String, value: String },
    #[error("at path {path_hint:?}: anon map keys must be parseable as unsigned integers")]
    InvalidAnonMapKey { path_hint: String },
    #[error("at path {path_hint:?}: init not a known value: \"{init:?}\"")]
    InitNotAKnownValue { path_hint: String, init: String },
    #[error("at path {path_hint:?}: value not within bounds")]
    ValueNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?}: only one variant is allowed")]
    OnlyOneVariantAllowed { path_hint: String },
    #[error("at path {path_hint:?}: enum items must be string")]
    EnumItemsMustBeString { path_hint: String },
    #[error("at path {path_hint:?} and attribute {attribute_name:?}: number must be finite")]
    NonFiniteNumber {
        path_hint: String,
        attribute_name: String,
    },
    #[error("at path {path_hint:?}: scale must be strictly positive")]
    ScaleMustBeStrictlyPositive { path_hint: String },
    #[error("received non-finite objective function value")]
    ObjFuncValMustBeFinite,
    #[error("no successfully evaluated individuals available")]
    NoIndividuals,
    #[error("client hung up")]
    ClientHungUp,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("unsuccessful termination of objective function child process.{}",
            .0.output.status.code().map(|code| format!(" Status code: {}", code))
            .unwrap_or_else(|| "".to_string()))]
    ObjFuncProcFailed(ProcOutputWithObjFuncArg),
    #[error("invalid output from objective function child process")]
    ObjFuncProcInvalidOutput(ProcOutputWithObjFuncArg),
    #[error("target objective function value must be finite")]
    TargetObjFuncValMustBeFinite,
    #[error("conflicting termination criteria")]
    ConflictingTerminationCriteria,
    #[error("output directory already exists")]
    OutputDirectoryAlreadyExists,
    #[error("failed to set signal handler")]
    FailedToSetSignalHandler(#[from] ctrlc::Error),
    #[error("failed to kill child process group. PID: {}", .0)]
    FailedToKillChildProcessGroup(Pid),
    #[error("failed to reap child process group. PID: {}", .0)]
    FailedToReapChildProcessGroup(Pid),
    #[error("quantile must be in [0, 1]")]
    InvalidQuantile,
    #[error("sample size must be strictly positive")]
    ZeroSampleSize,
    #[error("number of concurrent objective function evaluations must be strictly positive")]
    ZeroNumConcurrent,
    #[error("crossover probability must be in [0, 1]")]
    InvalidCrossoverProbability,
    #[error("selection pressure must be in [0, 1]")]
    InvalidSelectionPressure,
    #[error("mutation probability must be in [0, 1]")]
    InvalidMutationProbability,
    #[error("mutation scale must be strictly positive")]
    InvalidMutationScale,
}

#[derive(Debug)]
pub struct ProcOutputWithObjFuncArg {
    pub obj_func_arg: OsString,
    pub output: Output,
}

impl ProcOutputWithObjFuncArg {
    pub fn new(obj_func_arg: OsString, output: Output) -> Self {
        Self {
            obj_func_arg,
            output,
        }
    }
}
