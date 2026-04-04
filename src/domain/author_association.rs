/// GitHub の author_association フィールドの型安全な enum 表現。
///
/// GraphQL では `authorAssociation: CommentAuthorAssociation!` として返る文字列を
/// この型に変換して使用する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorAssociation {
    Owner,
    Member,
    Collaborator,
    Contributor,
    FirstTimer,
    FirstTimeContributor,
    None,
}

impl AuthorAssociation {
    /// `AuthorAssociation` を GitHub API 形式の文字列に変換する。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "OWNER",
            Self::Member => "MEMBER",
            Self::Collaborator => "COLLABORATOR",
            Self::Contributor => "CONTRIBUTOR",
            Self::FirstTimer => "FIRST_TIMER",
            Self::FirstTimeContributor => "FIRST_TIME_CONTRIBUTOR",
            Self::None => "NONE",
        }
    }
}

impl std::str::FromStr for AuthorAssociation {
    type Err = String;

    /// 文字列から `AuthorAssociation` に変換する（大文字小文字不問）。
    ///
    /// 未知の文字列は `Err` を返す。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "OWNER" => Ok(Self::Owner),
            "MEMBER" => Ok(Self::Member),
            "COLLABORATOR" => Ok(Self::Collaborator),
            "CONTRIBUTOR" => Ok(Self::Contributor),
            "FIRST_TIMER" => Ok(Self::FirstTimer),
            "FIRST_TIME_CONTRIBUTOR" => Ok(Self::FirstTimeContributor),
            "NONE" => Ok(Self::None),
            _ => Err(format!("invalid author association: {s}")),
        }
    }
}

/// association チェックの設定を表す sum type。
///
/// - `All`: チェックをスキップし、すべてのユーザーを信頼済みとして扱う
/// - `Specific(Vec<AuthorAssociation>)`: 指定した association を持つユーザーのみを信頼する
#[derive(Debug, Clone)]
pub enum TrustedAssociations {
    /// すべてのユーザーを信頼済みとして扱う（チェックスキップ）。
    All,
    /// 指定した association リストに含まれるユーザーのみを信頼する。
    Specific(Vec<AuthorAssociation>),
}

impl TrustedAssociations {
    /// `assoc` が信頼済みかどうかを返す。
    ///
    /// `All` の場合は常に `true`、`Specific` の場合はリストに含まれる場合のみ `true`。
    pub fn is_trusted(&self, assoc: &AuthorAssociation) -> bool {
        match self {
            Self::All => true,
            Self::Specific(list) => list.contains(assoc),
        }
    }

    /// 信頼済みの association リストを文字列スライスとして返す。
    ///
    /// `All` の場合は `None` を返す。
    pub fn as_display_list(&self) -> Option<Vec<&'static str>> {
        match self {
            Self::All => None,
            Self::Specific(list) => Some(list.iter().map(AuthorAssociation::as_str).collect()),
        }
    }
}

impl Default for TrustedAssociations {
    /// デフォルト値: `Specific([Owner, Member, Collaborator])`
    fn default() -> Self {
        Self::Specific(vec![
            AuthorAssociation::Owner,
            AuthorAssociation::Member,
            AuthorAssociation::Collaborator,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- AuthorAssociation::from_str ---

    #[test]
    fn from_str_valid_uppercase() {
        assert_eq!(
            "OWNER".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Owner
        );
        assert_eq!(
            "MEMBER".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Member
        );
        assert_eq!(
            "COLLABORATOR".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Collaborator
        );
        assert_eq!(
            "CONTRIBUTOR".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Contributor
        );
        assert_eq!(
            "FIRST_TIMER".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::FirstTimer
        );
        assert_eq!(
            "FIRST_TIME_CONTRIBUTOR"
                .parse::<AuthorAssociation>()
                .unwrap(),
            AuthorAssociation::FirstTimeContributor
        );
        assert_eq!(
            "NONE".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::None
        );
    }

    #[test]
    fn from_str_valid_lowercase() {
        assert_eq!(
            "owner".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Owner
        );
        assert_eq!(
            "member".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Member
        );
        assert_eq!(
            "collaborator".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Collaborator
        );
    }

    #[test]
    fn from_str_valid_mixed_case() {
        assert_eq!(
            "Owner".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Owner
        );
        assert_eq!(
            "Collaborator".parse::<AuthorAssociation>().unwrap(),
            AuthorAssociation::Collaborator
        );
    }

    #[test]
    fn from_str_invalid_returns_err() {
        assert!("ADMIN".parse::<AuthorAssociation>().is_err());
        assert!("".parse::<AuthorAssociation>().is_err());
        assert!("unknown".parse::<AuthorAssociation>().is_err());
        assert!("all".parse::<AuthorAssociation>().is_err());
    }

    #[test]
    fn from_str_error_message_contains_input() {
        let err = "INVALID_VALUE".parse::<AuthorAssociation>().unwrap_err();
        assert!(
            err.contains("INVALID_VALUE"),
            "error message should contain the invalid value"
        );
    }

    // --- TrustedAssociations::is_trusted ---

    #[test]
    fn all_trusts_every_association() {
        let trusted = TrustedAssociations::All;
        for assoc in [
            AuthorAssociation::Owner,
            AuthorAssociation::Member,
            AuthorAssociation::Collaborator,
            AuthorAssociation::Contributor,
            AuthorAssociation::FirstTimer,
            AuthorAssociation::FirstTimeContributor,
            AuthorAssociation::None,
        ] {
            assert!(trusted.is_trusted(&assoc), "All should trust {assoc:?}");
        }
    }

    #[test]
    fn specific_trusts_listed_associations() {
        let trusted = TrustedAssociations::Specific(vec![
            AuthorAssociation::Owner,
            AuthorAssociation::Member,
        ]);
        assert!(trusted.is_trusted(&AuthorAssociation::Owner));
        assert!(trusted.is_trusted(&AuthorAssociation::Member));
    }

    #[test]
    fn specific_rejects_unlisted_associations() {
        let trusted = TrustedAssociations::Specific(vec![
            AuthorAssociation::Owner,
            AuthorAssociation::Member,
        ]);
        assert!(!trusted.is_trusted(&AuthorAssociation::Collaborator));
        assert!(!trusted.is_trusted(&AuthorAssociation::Contributor));
        assert!(!trusted.is_trusted(&AuthorAssociation::FirstTimer));
        assert!(!trusted.is_trusted(&AuthorAssociation::FirstTimeContributor));
        assert!(!trusted.is_trusted(&AuthorAssociation::None));
    }

    #[test]
    fn specific_empty_rejects_all() {
        let trusted = TrustedAssociations::Specific(vec![]);
        assert!(!trusted.is_trusted(&AuthorAssociation::Owner));
        assert!(!trusted.is_trusted(&AuthorAssociation::Member));
    }

    // --- TrustedAssociations::default ---

    #[test]
    fn default_is_owner_member_collaborator() {
        let default = TrustedAssociations::default();
        assert!(default.is_trusted(&AuthorAssociation::Owner));
        assert!(default.is_trusted(&AuthorAssociation::Member));
        assert!(default.is_trusted(&AuthorAssociation::Collaborator));
        assert!(!default.is_trusted(&AuthorAssociation::Contributor));
        assert!(!default.is_trusted(&AuthorAssociation::FirstTimer));
        assert!(!default.is_trusted(&AuthorAssociation::FirstTimeContributor));
        assert!(!default.is_trusted(&AuthorAssociation::None));
    }
}
