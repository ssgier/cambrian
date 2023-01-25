use nix::unistd::Pid;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::Output;
use thiserror;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid YAML")]
    InvalidYaml(#[from] serde_yaml::Error),
    #[error("invalid JSON")]
    InvalidJson(#[from] serde_json::Error),
    #[error("at path {path_hint:?}: value must be a map")]
    ValueMustBeMap { path_hint: String },
    #[error(
        "at path {path_hint:?}: cannot parse attribute {attribute_name:?}. Number probably too big"
    )]
    UnsignedIntConversionFailed {
        path_hint: String,
        attribute_name: String,
    },
    #[error("at path {path_hint:?}: attribute value of {attribute_name:?} must be {} (make sure not to leave any trailing comma)", expected_type_hint)]
    InvalidAttributeValueType {
        path_hint: String,
        attribute_name: String,
        expected_type_hint: String,
    },
    #[error("at path {path_hint:?}: invalid attribute key: {}. Attribute keys must be of type string", .formatted_attribute_key)]
    InvalidAttributeKeyType {
        path_hint: String,
        formatted_attribute_key: String,
    },
    #[error("at path {path_hint:?}: unknown type name: {}", .unknown_type_name)]
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
    #[error("at path {path_hint:?}: mandatory attribute missing: {}", .missing_attribute_name)]
    MandatoryAttributeMissing {
        path_hint: String,
        missing_attribute_name: String,
    },
    #[error("at path {path_hint:?}: unexpected attribute: {}", .unexpected_attribute_name)]
    UnexpectedAttribute {
        path_hint: String,
        unexpected_attribute_name: String,
    },
    #[error("at path {path_hint:?}: sub must not be empty")]
    EmptySub { path_hint: String },
    #[error("at path {path_hint:?}: variant must have at least two variant values")]
    NotEnoughVariantValues { path_hint: String },
    #[error("at path {path_hint:?}: enum must have at least two enum values")]
    NotEnoughEnumValues { path_hint: String },
    #[error("at path {path_hint:?}: missing mandatory value")]
    MandatoryValueMissing { path_hint: String },
    #[error("at path {path_hint:?}: failed to convert number, not {}", .expected_type_hint)]
    NumberConversionFailed {
        path_hint: String,
        expected_type_hint: String,
    },
    #[error("at path {path_hint:?}: wrong type. Must be {}", .type_hint)]
    WrongTypeForValue {
        path_hint: String,
        type_hint: String,
    },
    #[error("at path {path_hint:?}: unexpected key: {}", .key)]
    UnexpectedKey { path_hint: String, key: String },
    #[error("at path {path_hint:?}: unknown variant: {}", .variant_name)]
    UnknownVariant {
        path_hint: String,
        variant_name: String,
    },
    #[error("at path {path_hint:?}: unknown enum value: {}", .value)]
    UnknownEnumValue { path_hint: String, value: String },
    #[error("at path {path_hint:?}: invalid anon map key: {}. Anon map keys must be unsigned integers",
            .invalid_key_formatted)]
    InvalidAnonMapKey {
        path_hint: String,
        invalid_key_formatted: String,
    },
    #[error("at path {path_hint:?}: init not a known value: {}", .init)]
    InitNotAKnownValue { path_hint: String, init: String },
    #[error("at path {path_hint:?}: value not within bounds")]
    ValueNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?}: exactly one variant value must be present, found {}", .num_variant_values_found)]
    ExactlyOneVariantValueRequired {
        path_hint: String,
        num_variant_values_found: usize,
    },
    #[error("at path {path_hint:?}: enum items must be string")]
    EnumItemsMustBeString { path_hint: String },
    #[error("at path {path_hint:?}, attribute {attribute_name:?}: number must be finite")]
    NonFiniteNumber {
        path_hint: String,
        attribute_name: String,
    },
    #[error("at path {path_hint:?}: scale must be strictly positive")]
    ScaleMustBeStrictlyPositive { path_hint: String },
    #[error(
        "at path {}: typeDef {}: must not use name of built-in type as name of user defined type",
        path_hint,
        type_def_name
    )]
    IllegalTypeDefName {
        path_hint: String,
        type_def_name: String,
    },
    #[error("received non-finite objective function value")]
    ObjFuncValMustBeFinite,
    #[error("no successfully evaluated individuals available")]
    NoIndividuals,
    #[error("client hung up")]
    ClientHungUp,
    #[error("unable to launch objective function child process: {}", .0)]
    UnableToLaunchObjFuncProcess(io::Error),
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
    ZeroSampleSize,
    #[error("number of concurrent objective function evaluations must be strictly positive")]
    ZeroNumConcurrent,
    #[error("Unable to create detailed reporting file at path: {}, cause: {}", .path.display(), .source)]
    UnableToCreateDetailedReportingFile {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub struct ProcOutputWithObjFuncArg {
    pub obj_func_arg: OsString,
    pub seed: u64,
    pub output: Output,
}

impl ProcOutputWithObjFuncArg {
    pub fn new(obj_func_arg: OsString, seed: u64, output: Output) -> Self {
        Self {
            obj_func_arg,
            seed,
            output,
        }
    }
}
