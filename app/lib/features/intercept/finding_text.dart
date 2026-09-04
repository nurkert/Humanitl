/// What a finding is called, and where it sits, in the person's language
/// (`docs/UX.md` 4.3 and 4.7).
///
/// The `kind` of a finding is internal vocabulary and never stands in a
/// sentence: `api_key:github` is what the daemon writes, "a GitHub API key" is
/// what a person reads. An unknown kind falls back to the general word rather
/// than printing the identifier -- a screen that shows `custom:acme` teaches
/// nothing (`docs/UX.md` 4.2).
///
/// Nothing here draws, so both sentences can be checked without a tree.
library;

import '../../core/domain/domain.dart';
import '../../l10n/l10n.dart';

/// The kind of [finding] without its parameter: `api_key:github` becomes
/// `api_key`.
String findingKind(Finding finding) {
  final int colon = finding.kind.indexOf(':');
  return colon < 0 ? finding.kind : finding.kind.substring(0, colon);
}

/// The parameter of [finding], or an empty string: the provider of an API key,
/// the term of a user term.
String findingParameter(Finding finding) {
  final int colon = finding.kind.indexOf(':');
  return colon < 0 ? '' : finding.kind.substring(colon + 1);
}

/// What [finding] is called, in the person's language.
///
/// The names are written to fit into both sentences of `docs/UX.md` 4.3 and
/// 4.7 without a change of case, so they start lowercase.
String findingName(Finding finding, AppLocalizations l10n) {
  final String parameter = findingParameter(finding);
  return switch (findingKind(finding)) {
    'api_key' =>
      parameter.isEmpty ? l10n.findingApiKey : l10n.findingApiKeyOf(parameter),
    'jwt' => l10n.findingJwt,
    'email' => l10n.findingEmail,
    'iban' => l10n.findingIban,
    'credit_card' => l10n.findingCreditCard,
    'phone' => l10n.findingPhone,
    'ipv4' => l10n.findingIpv4,
    'user_term' => l10n.findingUserTerm,
    _ => l10n.findingSecret,
  };
}

/// Where [finding] sits, as the sentence needs it: "in the body", "in the
/// `authorization` header".
String findingWhere(Finding finding, AppLocalizations l10n) =>
    switch (finding.location) {
      FindingLocation.header =>
        finding.headerName.isEmpty
            ? l10n.findingInHeader
            : l10n.findingInNamedHeader(finding.headerName),
      FindingLocation.query => l10n.findingInQuery,
      FindingLocation.body => l10n.findingInBody,
    };
