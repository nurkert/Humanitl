/// HTTP value types as the daemon reports them: methods, schemes, authorities,
/// headers and body references. Mirrors of `humanitl-core::HttpRequest` and of
/// `common.proto`.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

part 'http.freezed.dart';

/// HTTP method. Anything the daemon does not know is [other], with the raw
/// token next to it wherever a method travels.
enum Method {
  /// `GET`.
  get,

  /// `HEAD`.
  head,

  /// `POST`.
  post,

  /// `PUT`.
  put,

  /// `PATCH`.
  patch,

  /// `DELETE`.
  delete,

  /// `OPTIONS`.
  options,

  /// `CONNECT`.
  connect,

  /// `TRACE`.
  trace,

  /// Any other token; see the accompanying `methodRaw`.
  other;

  /// The uppercase token, or `?` for [other].
  String get token => this == Method.other ? '?' : name.toUpperCase();

  /// The token to display: the raw value for [other], otherwise [token].
  String display(String raw) =>
      this == Method.other && raw.isNotEmpty ? raw.toUpperCase() : token;
}

/// Scheme of the target URL.
enum Scheme {
  /// Plain HTTP.
  http,

  /// HTTPS.
  https,

  /// WebSocket over plain TCP.
  ws,

  /// WebSocket over TLS.
  wss;

  /// The default port of this scheme.
  int get defaultPort => switch (this) {
    Scheme.http || Scheme.ws => 80,
    Scheme.https || Scheme.wss => 443,
  };
}

/// Protocol upgrade of a request.
enum Upgrade {
  /// No upgrade, the normal case.
  none,

  /// A WebSocket upgrade; matches only rules with `upgrade: websocket`.
  websocket,
}

/// Target of a request, normalised (A-label, lowercase, no trailing dot).
@freezed
abstract class Authority with _$Authority {
  /// Creates an authority.
  const factory Authority({
    required String host,
    required int port,
    @Default(false) bool isIpLiteral,
    @Default('') String displayHost,
  }) = _Authority;

  const Authority._();

  /// The host as the human should read it: the U-label when the daemon
  /// supplied one, otherwise [host].
  String get shownHost => displayHost.isEmpty ? host : displayHost;

  /// `host:port`, with the port left out when it is the default of [scheme].
  String display(Scheme scheme) =>
      port == scheme.defaultPort ? shownHost : '$shownHost:$port';
}

/// One HTTP header. The value is bytes because headers are not guaranteed to
/// be UTF-8.
@freezed
abstract class Header with _$Header {
  /// Creates a header.
  const factory Header({required String name, required List<int> value}) =
      _Header;

  const Header._();

  /// The value decoded as UTF-8, invalid bytes replaced.
  String get text => String.fromCharCodes(value);
}

/// Reference to a body. Bodies never travel inline in events; the content
/// comes through `GetBody`.
@freezed
abstract class BodyRef with _$BodyRef {
  /// Creates a body reference.
  const factory BodyRef({
    required List<int> sha256,
    required int size,
    @Default(false) bool truncated,
    @Default('') String contentType,
  }) = _BodyRef;

  const BodyRef._();

  /// True when the body is empty and there is nothing to fetch.
  bool get isEmpty => size == 0;
}

/// A complete request without its body content.
@freezed
abstract class HttpRequest with _$HttpRequest {
  /// Creates a request.
  const factory HttpRequest({
    required Method method,
    @Default('') String methodRaw,
    required Scheme scheme,
    required Authority authority,
    required String pathAndQuery,
    @Default(<Header>[]) List<Header> headers,
    required BodyRef body,
    @Default('') String version,
  }) = _HttpRequest;

  const HttpRequest._();

  /// The method token to show.
  String get methodLabel => method.display(methodRaw);
}

/// Status line and headers of a response.
@freezed
abstract class HttpResponseHead with _$HttpResponseHead {
  /// Creates a response head.
  const factory HttpResponseHead({
    required int status,
    @Default(<Header>[]) List<Header> headers,
    @Default('') String version,
  }) = _HttpResponseHead;
}

/// A request as the human edited it: the one place a body travels towards the
/// daemon (CONVENTIONS 4.11).
@freezed
abstract class EditedRequest with _$EditedRequest {
  /// Creates an edited request.
  const factory EditedRequest({
    required Method method,
    @Default('') String methodRaw,
    required String url,
    @Default(<Header>[]) List<Header> headers,
    @Default(<int>[]) List<int> body,
  }) = _EditedRequest;
}
