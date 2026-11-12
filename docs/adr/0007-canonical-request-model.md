# ADR 0007: A canonical request model between surfaces and providers

Status: accepted

## Context

Callers speak four wire formats: OpenAI Chat Completions, OpenAI
Responses, Anthropic Messages and Gemini. Providers speak about as
many. Translating each pair directly needs a converter per pair and
makes every routing or masking feature format-specific.

## Decision

Every inbound surface decodes into one canonical model (`ChatRequest`,
`ChatEvent`, spec, router, section 1) and every provider adapter
encodes from it. Routing, masking, guardrails, usage and tools work on
the canonical model only. A streaming response is a stream of
canonical events. A non-streaming response is the fold of that stream.

Fields a surface has and the model lacks travel in `PassThrough`, and
are forwarded only when the provider speaks the caller's own format.

## Alternatives considered

- **Pairwise translation:** quadratic work, and features drift between
  pairs.
- **Use one vendor's format as the model:** every other vendor's
  features become awkward extensions of a format riehub does not own.

## Proof

Task T-0208 runs a matrix: each surface against each provider kind,
for text, tool calls, images, reasoning and streaming, and checks that
decode then encode in the same format is the identity on the fixtures.

## Consequences

A feature that exists in only one format can be lost in translation
across formats. The matrix test lists exactly which. New surfaces and
providers cost one codec each.
