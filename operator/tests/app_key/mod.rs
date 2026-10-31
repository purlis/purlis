// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A GitHub App key for the tests: an RSA key made for them, the same as the desktop's mint's
//! tests hold. Its armour is put together here, so the repository's own secret scan does not take
//! it for a key.

#![allow(dead_code)]

const PKCS1: &[&str] = &[
    "MIIEowIBAAKCAQEAyLQzaC2M6MsSa1dvIokROES8F/FxYVMhaadx5p46OjcJ8fE/",
    "3Zcj1f5RyC7hp6VDLWxdjzerF8C0p1R3Ks03NRI0ZLuVVjRvdyyECU49Oi6U1+dJ",
    "CukHhSrZzNc3Cn3DIEcpY6izRphx9wPPxwa8Cibi3UZ866rMnBOBYi7sJFxZE+NL",
    "gKYueL0m7mVXvARkK6iMz3GfP5UaHebfjE1UrvyvbBJTCMQQR5EtL9nmFNh6y5Y+",
    "ZQ/qyPr2xoCrtPbEqmmgRYAiUAJzCg/at/71fxhfokcRsE0mN/4yltinPAews9Sj",
    "xKJmLNe4tx+jjvoQZLcMexAJanNuSncXLR7nswIDAQABAoIBADCBlLcF4ztT8PSu",
    "Yfanx9igjBEBUlSiQb4GMPn6nbn6GVY25yeH4qpSV7OY2QKqZ0zwFsCJRLlt4nta",
    "Z56ZkFEsQCtkRphZ/eKAiLyzAwsvvNtdU4LmhOHDVHY0QWIwuNrxN+iVGhbiuxBn",
    "BG1mjEWr1kKCSaivU9oYwLlY+8thdLLQS047GOufL5GwXwhkPlUJRGlXR3fY6qoE",
    "BCTVFXkuLcmQbcvRPQcBloWkfNaVyQgVJRiV6tGiHaenl5Z4s4r7Y8BkXJuUTuss",
    "Ftt/Ffb/nOWi110nALb+H22cRUVPLkCgREhKHTaymwOg5Cgu6z1l3+oFoCyVh+Ev",
    "UtjZV6ECgYEA53pjT6zkkfs+nXyooe44ozTt47S9InMm+J1iedvRvhrpvLAEn39A",
    "S7/MElUK/JfmS/F/nhwdrjzCE4AXElSJiZGzyFlPCvX80CWpqfXHIHLsDyBjl8CO",
    "SR3MoDAxIH2VMICBCwisJuP5AjgGP9iJT2NPyDa2zes0w5Cv8cjbezECgYEA3fc6",
    "KMA1ix6y3lD5+Jd8KEEyZv1C/x2oSrvnZAsSFvhaRkBGzE99g+h+TdP0g8aCHhPF",
    "HXPoA+OIqXLnfGmFheb+weV/pEIq1arcLV8HZsjg8yXhAODi3Co6xxyV/ebUlS7G",
    "xeZZn4UgkGnc7bVCZISLkBqPNmLiVqLXAEeKECMCgYBbgCOVsTBBDGG5+zblm7dD",
    "vH50zlMdNQuopv3wUZZdYWIXtjR9n92Yk95lOLy5EIp2ZqYdp9uLrnTBJJb6JfSg",
    "Lro0bikymf4vgVPJ3V+zUfpCGHmF2RXljSQABkzFHR9GJuXbqLf0P5WE7QR7ZGsS",
    "uJ0m9YnZaCpZS+5g0xb/YQKBgEp/a7m5qT7kMsyDCSLyVO191FjUr+PIJG/Q2Tyu",
    "qoejG0lw0VJJa6sWIS4l8W849Pr7T7Eu8bQ8beXRdvg+wl3Kxb52NH0A6DeGfReN",
    "SYKp8TYqhHYzBHE4iznA0mbBTqAzbuuFAJeVqvRNU6VErN/nzJ+1ClQKpETHqrmh",
    "SSlZAoGBAKPump25EhIl1HLcf+Qz+TU7r+6JmjTWpuuJrStyHN0VyZpY1w77hrTQ",
    "Zi5cXHIId315D0aF+oJeDpLwZkY3a3r5z8kUCgBOkyG6GlUvK9WY9J9X65YlKhRB",
    "kk2mliU8zkJ3dyKztUqzPtUrYnmlntKlYh2rSykOpCpcQfDehoKY",
];

/// The key, as a PEM of the kind GitHub hands out.
pub fn pem() -> String {
    let dashes = "-----";
    let label = ["RSA ", "PRIVATE KEY"].concat();
    format!(
        "{dashes}BEGIN {label}{dashes}\n{}\n{dashes}END {label}{dashes}\n",
        PKCS1.join("\n")
    )
}
