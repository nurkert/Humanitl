# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

The public website humanitl.de: Astro, static output in `website/`, deployed to
GitHub Pages by a GitHub Actions workflow, modelled on the owner's whowolf.de
(`~/Documents/Development/Workspaces/Dart/whowolf/website`). German under `/`,
English under `/en/`. The product itself is a Rust daemon and CLI plus a
Flutter desktop app; their design system is `docs/DESIGN.md`.

## Users

Developers who let an AI coding agent work on their machine and want to see
and decide what it sends to the internet. They evaluate Humanitl on the
website, then download it for Linux or macOS.

## Product Purpose

Humanitl lets an AI coding agent reach the internet from a sandbox, one
approved request at a time. The agent runs without a network interface; its
only way out is a moderating proxy on the host. A request no rule decides is
held and shown with headers, parameters and body, and goes on only after the
user approves it. Everything is recorded.

## Positioning

Not a firewall and not a filter list: physics instead of trust. The sandbox
has no way out except one door, and behind that door a human decides. Three
guarantees, each checkable with one command (`humanitl sandbox check`).

## Operating Context

Local, on the developer's own machine: a daemon (`humanitld`), the command
line (`humanitl`) and the desktop app. Linux (bubblewrap, seccomp) and macOS
(Seatbelt, since 0.0.12). Open source on GitHub (nurkert/Humanitl).

## Capabilities and Constraints

- Releases 0.0.x on GitHub: `.deb` and a tarball for Linux, `.dmg` and `.zip`
  for macOS (universal, ad-hoc signed; first start needs "Open Anyway").
- App UI in English and German.
- The website's first version: a landing section and a screen carousel, plus
  imprint and privacy. Download and documentation pages follow later.

## Brand Commitments

- Name: Humanitl (human in the loop). Icon: a black fist on a yellow rounded
  tile (`app/macos/Runner/Assets.xcassets/AppIcon.appiconset`).
- App direction "Airlock" (`docs/DESIGN.md`): calm control room, near-black
  neutral grey surfaces (#111111, HUM-355), yellow accent #E8B84B, the yellow hazard
  stripe above the queue.
- Website look requested by the owner on 2026-10-30: a highly modern, clean
  web app; dark space, floating 3D glass screens with specular rim light,
  chrome headline, outlined pill buttons, screens that animate in on load and
  rearrange on scroll, an infinitely scrolling carousel.

## Evidence on Hand

- Real screens of the macOS app, captured against `humanitld --fake
  fixtures/sessions/npm-install.jsonl`: intercept, history, rules, sandbox,
  command palette, settings.
- No users, testimonials, benchmarks or press exist; none may be invented.
- The imprint lives at https://nurkert.de/impressum.

## Product Principles

1. Truth over persuasion: every claim on the site is one the code and
   `docs/SECURITY.md` back.
2. Show the product, not an illustration of it: real screens only.
3. The human decides: the held request is the centre of every story.
4. Local and open: no account, no cloud, source on GitHub.
