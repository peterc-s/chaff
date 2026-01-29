# What is Chaff?
Chaff is a framework for developing and using Website Fingerprinting (WF) defences.

## Key Aims
- Improve developer experience over prior works, such as Maybenot[^1] and WFDefProxy[^2].
- Allow defence developers to create defences that are:
    - Auditable
    - Portable
    - Expressive
- Integrate security estimation techniques so defence developers can test their defences for
  robustness.
- FFI for languages such as Python and C in the hopes that:
    - It makes training ML-based defences easier through the vast number of ML libraries Python has
      to offer.
    - Integration into existing Privacy Enhancing Technologies (PETs) such as Tor becomes easy.

# Design
## Motivation
- Some existing frameworks such as Maybenot and the Tor circuit padding framework[^3] limit
  developers in what they may implement by representing defences as state machines. While this makes
  machines easily auditable and, in the case of the Tor circuit padding framewor, genetically
  trainable[^4], it makes certain classes of defences impossible to implement faithfully (as seen in
  the Maybenot paper).
- Other frameworks are now outdated and [include known unsafe libraries](https://github.com/websitefingerprinting/wfdef/blob/3fee6d65d049488584a73db0bb80f071af19d259/common/ntor/ntor.go#L207).
- Developing a tool that allows WF researchers to both implement defences and test them, producing
  sound analytics, may accelerate WF defence research.

## Design Philosophy
- Auditability:
    - The core of Chaff should not rely on countless large libraries with many dependencies of their
      own. Library usage must be thoughtful and easily auditable.
- Portability/Deployability:
    - Chaff defences should be easily transferable between machines. This means that, for example,
      deploying a Chaff defence as a Tor Pluggable Transport (PT) must be easy to do.
- Expressiveness:
    - Ideally, Chaff should allow the defence developer to create whatever they can imagine as an
      effective defence without friction.
- Modularity:
    - Chaff should be extensible via modules or plugins with the aim of keeping up with advances in
      WF defences.
- Quality:
    - Code should be of high quality and easily readable (therefore auditable).
    - Chaff should undergo intensive testing and fuzzing.
    - Written in a memory safe language with any FFI tested properly and documented thoroughly.

[^1]: https://github.com/maybenot-io/maybenot
[^2]: https://github.com/websitefingerprinting/wfdef
[^3]: https://gitlab.torproject.org/tpo/core/tor/-/blob/main/doc/HACKING/CircuitPaddingDevelopment.md
[^4]: https://github.com/pylls/padding-machines-for-tor/tree/master/machines/phase2
