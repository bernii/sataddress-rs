<div id="top"></div>

<!-- PROJECT SHIELDS -->
<!--
*** I'm using markdown "reference style" links for readability.
*** Reference links are enclosed in brackets [ ] instead of parentheses ( ).
*** See the bottom of this document for the declaration of the reference variables
*** for contributors-url, forks-url, etc. This is an optional, concise syntax you may use.
*** https://www.markdownguide.org/basic-syntax/#reference-style-links
-->
[![Contributors][contributors-shield]][contributors-url]
[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![Build Status][build-status]][build-status-url]
[![MIT License][license-shield]][license-url]
[![LinkedIn][linkedin-shield]][linkedin-url]


<br />
<div align="center">
  <a href="https://github.com/bernii/sataddress-rs">
    <img src="https://raw.githubusercontent.com/bernii/sataddress-rs/7a09f16a116f1211d1e961bc7e78a1add88f6a4e/assets/inv_banner.png" alt="SatAddress RS logo" width="80">
  </a>

<h2 align="center">Lightning address federated server implementation in Rust</h3>
  <p align="center">
    <a href="https://satspay.to"><strong>Live Version</strong></a> | 
    <a href="https://docs.rs/sataddress/latest/sataddress/index.html"><strong>Documentation</strong></a>
    <br />
    <br />
    <a href="https://crates.io/crates/sataddress">Crates.io</a>
    ·
    <a href="https://github.com/bernii/sataddress-rs/issues">Report a Bug</a>
    ·
    <a href="https://github.com/bernii/sataddress-rs/issues">Feature Request</a>
  </p>
</div>


## About The Project

This is a [rust](https://www.rust-lang.org/) implementation of Federated [Lightning Address](https://lightningaddress.com/) Server.

Lightning address / alias helps with greatly improving the user experience of using LN payments by using email-like addresses for recieving and sending bitcoin lightning payments.

The federated server allows you to easily handle LN Address requests and add those capabilties to the domains you own.

The project consists of **server** and **cli** tool:
* **Server** is responsible for handling requests from *LN wallets* and serving the alias reservation page and APIs.
* **CLI tool** can be used to interact with the embedded database in order to export/import data or generate usage statistics.

## Getting Started

First, check out the *latest deployed version* at [satspay.to](https://satspay.to/)

The easiest way to run the server is just using the automatically published docker container.

You can configure the container easily by providing enivronment variables either by passing them to docker or by putting them into `dot-env` file.

```
# .env file
DOMAINS=sataddress.rs,another-domain.com
PIN_SECRET=my-secret-phrase
SITE_NAME=SATADDRESS
SITE_SUB_NAME=.rs
```

Once you have your config figured out, just run the container:

```bash
$ docker run -v $(pwd)/.env:/opt/sataddress/.env -v $(pwd)/sataddress.db:/opt/sataddress/sataddress.db --name sataddress -it --rm sataddress:latest
```

As an alternative, if you're familiar with the rust toolset, you can use [just](https://github.com/casey/just) which will also automatically load your `.env` file. 
```bash
$ just run
```

## Roadmap

- [ ] improve tests
- [ ] add REST API functionality for data manipulation
- [ ] better error generation & handling
- [ ] customizable image, memo, max/min invoice sats
- [ ] implementation for more backends/nodes (contributions welcome!)

See the [open issues](https://github.com/bernii/sataddress-rs/issues) for a full list of proposed features (and known issues).


## License

Distributed under the MIT License. See `LICENSE` for more information.


## Contact

Bernard Kobos - [@bkobos](https://twitter.com/bkobos) - bkobos+nospam!@gmail.com

Project Link: [https://github.com/bernii/sataddress-rs](https://github.com/bernii/sataddress-rs)

## Acknowledgments

* [sataddress](https://github.com/nbd-wtf/satdress) original federated lightning address server implementation which this implementation is based on
* [go-lnurl](https://github.com/fiatjaf/go-lnurl) which was helpful for learning about LN URL structures
* [Lightning Address](https://github.com/andrerfneves/lightning-address) documentation and explanations
* [BTC lightning logo](https://github.com/shocknet/bitcoin-lightning-logo) for creating an open source vector btc logo


<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[contributors-shield]: https://img.shields.io/github/contributors/bernii/sataddress-rs.svg?style=for-the-badge
[contributors-url]: https://github.com/bernii/sataddress-rs/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/bernii/sataddress-rs.svg?style=for-the-badge
[forks-url]: https://github.com/bernii/sataddress-rs/network/members
[stars-shield]: https://img.shields.io/github/stars/bernii/sataddress-rs.svg?style=for-the-badge
[stars-url]: https://github.com/bernii/sataddress-rs/stargazers
[issues-shield]: https://img.shields.io/github/issues/bernii/sataddress-rs.svg?style=for-the-badge
[issues-url]: https://github.com/bernii/sataddress-rs/issues
[license-shield]: https://img.shields.io/github/license/bernii/sataddress-rs.svg?style=for-the-badge
[license-url]: https://github.com/bernii/sataddress-rs/blob/main/LICENSE
[linkedin-shield]: https://img.shields.io/badge/-LinkedIn-black.svg?style=for-the-badge&logo=linkedin&colorB=555
[linkedin-url]: https://linkedin.com/in/bernii
[product-screenshot]: images/screenshot.png
[build-status]: https://img.shields.io/endpoint.svg?url=https%3A%2F%2Factions-badge.atrox.dev%2Fbernii%2Fsataddress-rs%2Fbadge%3Fref%3Dmain&style=for-the-badge
[build-status-url]: https://actions-badge.atrox.dev/bernii/sataddress-rs/goto?ref=main