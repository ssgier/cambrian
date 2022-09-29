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
    #[error("at path {path_hint:?}: attribute value of \"{attribute_name:?}\" must be {expected_type_hint:?}")]
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
    #[error("at path {path_hint:?}: unknown value \"{unknown_value:?}\" for probability distribution (\"prob_dist\")")]
    UnknownProbDist {
        path_hint: String,
        unknown_value: String,
    },
    #[error("at path {path_hint:?}: initial value is not within bounds provided")]
    InitNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?}: initial size is not within bounds provided")]
    InitSizeNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?}: min must be lower than max")]
    InvalidBounds { path_hint: String },
    #[error("at path {path_hint:?}: min size must be lower than max size")]
    InvalidSizeBounds { path_hint: String },
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
    #[error("at path {path_hint:?}: anon map keys must be parseable as unsigned integers")]
    InvalidAnonMapKey { path_hint: String },
    #[error("at path {path_hint:?}: value not within bounds")]
    ValueNotWithinBounds { path_hint: String },
    #[error("at path {path_hint:?} and attribute {attribute_name:?}: number must be finite")]
    NonFiniteNumber {
        path_hint: String,
        attribute_name: String,
    },
}
