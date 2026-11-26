use thiserror::Error;

pub type Result<T> = std::result::Result<T, PatchError>;

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("line numbers are one-based")]
    InvalidLineNumber,
    #[error("patch is empty")]
    PatchIsEmpty,
    #[error("expected exactly one file patch")]
    MultipleFiles,
    #[error("duplicate EOF marker")]
    DuplicateEofMarker,
    #[error("unsupported patch body")]
    UnsupportedPatchBody,
    #[error("patch body line is not terminated")]
    PatchBodyLineIsNotTerminated,
    #[error("hunk counts do not match body")]
    HunkCountMismatch,
    #[error("empty selection")]
    EmptySelection,
    #[error("selection is outside the patch")]
    SelectionIsOutsideThePatch,
    #[error("binary patches cannot be reversed")]
    BinaryReverseUnsupported,
    #[error("metadata-only patches cannot be reversed")]
    MetadataReverseUnsupported,
    #[error("only regular empty files can be reversed")]
    EmptyFileModeUnsupported,
    #[error("selection contains no changed lines")]
    NoChangedLines,
    #[error("selection creates content after original EOF without a newline")]
    OriginalEofConflict,
    #[error("selection creates content after modified EOF without a newline")]
    ModifiedEofConflict,
    #[error("invalid partial hunk coordinates")]
    InvalidPartialHunkCoordinates,
    #[error("unsupported Git path quoting")]
    UnsupportedPathQuoting,
    #[error("selection anchor is outside the patch")]
    SelectionAnchorIsOutsideThePatch,
    #[error("selection head is outside the patch")]
    SelectionHeadIsOutsideThePatch,
    #[error("orphan EOF marker")]
    OrphanEofMarker,
    #[error("empty patch line")]
    EmptyPatchLine,
    #[error("original coordinate overflow")]
    OriginalCoordinateOverflow,
    #[error("modified coordinate overflow")]
    ModifiedCoordinateOverflow,
    #[error("missing hunk")]
    MissingHunk,
    #[error("patch has no source lines")]
    PatchHasNoSourceLines,
    #[error("missing empty file delta")]
    MissingEmptyFileDelta,
    #[error("missing Git header")]
    MissingGitHeader,
    #[error("missing empty file mode")]
    MissingEmptyFileMode,
    #[error("missing empty file index")]
    MissingEmptyFileIndex,
    #[error("invalid empty file index")]
    InvalidEmptyFileIndex,
    #[error("missing empty original path")]
    MissingEmptyOriginalPath,
    #[error("missing empty modified path")]
    MissingEmptyModifiedPath,
    #[error("missing original path")]
    MissingOriginalPath,
    #[error("missing modified path")]
    MissingModifiedPath,
    #[error("invalid original path")]
    InvalidOriginalPath,
    #[error("invalid modified path")]
    InvalidModifiedPath,
    #[error("invalid hunk header")]
    InvalidHunkHeader,
    #[error("invalid hunk coordinates")]
    InvalidHunkCoordinates,
    #[error("invalid hunk label")]
    InvalidHunkLabel,
    #[error("{operation}")]
    Git { operation: &'static str, source: git2::Error },
    #[error("invalid patch coordinate {value}")]
    Coordinate { value: String, source: std::num::ParseIntError },
    #[error(transparent)]
    Partial { source: PartialPatchError },
}

impl PatchError {
    pub fn invalid_line_number() -> Self {
        Self::InvalidLineNumber
    }
    pub fn patch_is_empty() -> Self {
        Self::PatchIsEmpty
    }
    pub fn multiple_files() -> Self {
        Self::MultipleFiles
    }
    pub fn duplicate_eof_marker() -> Self {
        Self::DuplicateEofMarker
    }
    pub fn unsupported_patch_body() -> Self {
        Self::UnsupportedPatchBody
    }
    pub fn patch_body_line_is_not_terminated() -> Self {
        Self::PatchBodyLineIsNotTerminated
    }
    pub fn hunk_count_mismatch() -> Self {
        Self::HunkCountMismatch
    }
    pub fn empty_selection() -> Self {
        Self::EmptySelection
    }
    pub fn selection_is_outside_the_patch() -> Self {
        Self::SelectionIsOutsideThePatch
    }
    pub fn binary_reverse_unsupported() -> Self {
        Self::BinaryReverseUnsupported
    }
    pub fn metadata_reverse_unsupported() -> Self {
        Self::MetadataReverseUnsupported
    }
    pub fn empty_file_mode_unsupported() -> Self {
        Self::EmptyFileModeUnsupported
    }
    pub fn no_changed_lines() -> Self {
        Self::NoChangedLines
    }
    pub fn original_eof_conflict() -> Self {
        Self::OriginalEofConflict
    }
    pub fn modified_eof_conflict() -> Self {
        Self::ModifiedEofConflict
    }
    pub fn invalid_partial_hunk_coordinates() -> Self {
        Self::InvalidPartialHunkCoordinates
    }
    pub fn unsupported_path_quoting() -> Self {
        Self::UnsupportedPathQuoting
    }
    pub fn selection_anchor_is_outside_the_patch() -> Self {
        Self::SelectionAnchorIsOutsideThePatch
    }
    pub fn selection_head_is_outside_the_patch() -> Self {
        Self::SelectionHeadIsOutsideThePatch
    }
    pub fn orphan_eof_marker() -> Self {
        Self::OrphanEofMarker
    }
    pub fn empty_patch_line() -> Self {
        Self::EmptyPatchLine
    }
    pub fn original_coordinate_overflow() -> Self {
        Self::OriginalCoordinateOverflow
    }
    pub fn modified_coordinate_overflow() -> Self {
        Self::ModifiedCoordinateOverflow
    }
    pub fn missing_hunk() -> Self {
        Self::MissingHunk
    }
    pub fn patch_has_no_source_lines() -> Self {
        Self::PatchHasNoSourceLines
    }
    pub fn missing_empty_file_delta() -> Self {
        Self::MissingEmptyFileDelta
    }
    pub fn missing_git_header() -> Self {
        Self::MissingGitHeader
    }
    pub fn missing_empty_file_mode() -> Self {
        Self::MissingEmptyFileMode
    }
    pub fn missing_empty_file_index() -> Self {
        Self::MissingEmptyFileIndex
    }
    pub fn invalid_empty_file_index() -> Self {
        Self::InvalidEmptyFileIndex
    }
    pub fn missing_empty_original_path() -> Self {
        Self::MissingEmptyOriginalPath
    }
    pub fn missing_empty_modified_path() -> Self {
        Self::MissingEmptyModifiedPath
    }
    pub fn missing_original_path() -> Self {
        Self::MissingOriginalPath
    }
    pub fn missing_modified_path() -> Self {
        Self::MissingModifiedPath
    }
    pub fn invalid_original_path() -> Self {
        Self::InvalidOriginalPath
    }
    pub fn invalid_modified_path() -> Self {
        Self::InvalidModifiedPath
    }
    pub fn invalid_hunk_header() -> Self {
        Self::InvalidHunkHeader
    }
    pub fn invalid_hunk_coordinates() -> Self {
        Self::InvalidHunkCoordinates
    }
    pub fn invalid_hunk_label() -> Self {
        Self::InvalidHunkLabel
    }
    pub fn git(operation: &'static str, source: git2::Error) -> Self {
        Self::Git { operation, source }
    }
    pub fn coordinate(value: &str, source: std::num::ParseIntError) -> Self {
        Self::Coordinate { value: value.to_owned(), source }
    }
    pub fn partial(source: PartialPatchError) -> Self {
        Self::Partial { source }
    }
}

#[derive(Clone, Debug, Error)]
pub enum PartialPatchError {
    #[error("binary patches do not support partial operations")]
    Binary,
    #[error("rename and type changes do not support partial operations")]
    Delta,
    #[error("symlinks and submodules do not support partial operations")]
    FileMode,
    #[error("metadata and binary changes do not support partial operations")]
    Metadata,
    #[error("patch has no text hunks")]
    NoTextHunks,
}
impl PartialPatchError {
    pub fn binary() -> Self {
        Self::Binary
    }
    pub fn delta() -> Self {
        Self::Delta
    }
    pub fn file_mode() -> Self {
        Self::FileMode
    }
    pub fn metadata() -> Self {
        Self::Metadata
    }
    pub fn no_text_hunks() -> Self {
        Self::NoTextHunks
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Binary => "binary patches do not support partial operations",
            Self::Delta => "rename and type changes do not support partial operations",
            Self::FileMode => "symlinks and submodules do not support partial operations",
            Self::Metadata => "metadata and binary changes do not support partial operations",
            Self::NoTextHunks => "patch has no text hunks",
        }
    }
}
