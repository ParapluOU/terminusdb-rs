---
tags:
  - explanation
  - enterprise
  - advanced
title: Operational Technologies Information Transfer
nextjs:
  metadata:
    title: Operational Technologies Information Transfer
    description: How to use TerminusDB in Operational Technologies environments for IEC62443, Purdue model and other environments where network segmentation is strict.
    keywords: terminusdb, operational technologies information transfer
    alternates:
      canonical: https://terminusdb.org/docs/operational-technologies-transfer/
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
media: []
---

## Transfer data in operational technologies landscapes

TerminusDB is designed for many different kinds of environments, including cloud, local, firewalled, DMZ and airgapped environments. The git-for-data features enables knowledge graphs stored in TerminusDB to be transferred between environments that have special communications requirements.

In IEC62443, purdue network and other environments, communications are only allowed to move outwards in an onion-layered model, where the outermost layer of the operational technology environment is a DMZ that only allows inbound communications both from an IT environment and the OT environments, preventing all outbound communications.

TerminusDB is well positioned for data transports in such environments thanks to the bidirectional `push` and `pull` protocols to transport the latest information as layer updates and where necessary also specific branches.

An analytics data product can be pushed in layers (between TerminusDB instances) all the way to a DMZ network handoff point, and then `pull`ed from the DMZ into the IT environment. This prevents any outbound communication from the DMZ environment, whilst the information can flow as part of a digital twin of the operational environment.

Read more about the git-for-data of knowledge graphs in the [Git-for-Data Reference](/docs/git-for-data-reference/) page.