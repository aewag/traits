//! JSON Web Key (JWK) Support.
//!
//! Specified in RFC 7518 Section 6: Cryptographic Algorithms for Keys:
//! <https://tools.ietf.org/html/rfc7518#section-6>

use crate::{
    sec1::{
        Coordinates, EncodedPoint, UncompressedPointSize, UntaggedPointSize, ValidatePublicKey,
    },
    secret_key::{SecretKey, SecretValue},
    weierstrass::Curve,
    Error, FieldBytes,
};
use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
};
use core::{
    convert::{TryFrom, TryInto},
    fmt::{self, Debug},
    marker::PhantomData,
    ops::Add,
    str::{self, FromStr},
};
use generic_array::{typenum::U1, ArrayLength};
use serde::{de, ser, Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

#[cfg(feature = "arithmetic")]
use crate::{
    ff::PrimeField,
    public_key::PublicKey,
    sec1::{FromEncodedPoint, ToEncodedPoint},
    AffinePoint, ProjectiveArithmetic, ProjectivePoint, Scalar,
};

/// Key Type (`kty`) for elliptic curve keys.
pub const EC_KTY: &str = "EC";

/// Deserialization error message.
const DE_ERROR_MSG: &str = "struct JwkEcKey with 5 elements";

/// Name of the JWK type
const JWK_TYPE_NAME: &str = "JwkEcKey";

/// Field names
const FIELDS: &[&str] = &["kty", "crv", "x", "y", "d"];

/// Elliptic curve parameters used by JSON Web Keys.
#[cfg_attr(docsrs, doc(cfg(feature = "jwk")))]
pub trait JwkParameters: Curve {
    /// The `crv` parameter which identifies a particular elliptic curve
    /// as defined in RFC 7518 Section 6.2.1.1:
    /// <https://tools.ietf.org/html/rfc7518#section-6.2.1.1>
    ///
    /// Curve values are registered in the IANA "JSON Web Key Elliptic Curve"
    /// registry defined in RFC 7518 Section 7.6:
    /// <https://tools.ietf.org/html/rfc7518#section-7.6>
    const CRV: &'static str;
}

/// JWK elliptic curve keys with a `kty` of `"EC"`.
///
/// They can represent either a public/private keypair, or just a public key,
/// depending on whether or not the `d` parameter is present.
// TODO(tarcieri): eagerly decode or validate `x`, `y`, and `d` as Base64
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "jwk")))]
pub struct JwkEcKey {
    /// The `crv` parameter which identifies a particular elliptic curve
    /// as defined in RFC 7518 Section 6.2.1.1:
    /// <https://tools.ietf.org/html/rfc7518#section-6.2.1.1>
    crv: String,

    /// The x-coordinate of the elliptic curve point which is the public key
    /// value associated with this JWK as defined in RFC 7518 6.2.1.2:
    /// <https://tools.ietf.org/html/rfc7518#section-6.2.1.2>
    x: String,

    /// The y-coordinate of the elliptic curve point which is the public key
    /// value associated with this JWK as defined in RFC 7518 6.2.1.3:
    /// <https://tools.ietf.org/html/rfc7518#section-6.2.1.3>
    y: String,

    /// The `d` ECC private key parameter as described in RFC 7518 6.2.2.1:
    /// <https://tools.ietf.org/html/rfc7518#section-6.2.2.1>
    ///
    /// Value is optional and if omitted, this JWK represents a private key.
    ///
    /// Inner value is encoded according to the `Integer-to-Octet-String`
    /// conversion as defined in SEC1 section 2.3.7:
    /// <https://www.secg.org/sec1-v2.pdf>
    d: Option<String>,
}

impl JwkEcKey {
    /// Get the `crv` parameter for this JWK.
    pub fn crv(&self) -> &str {
        &self.crv
    }

    /// Is this JWK a keypair that includes a private key?
    pub fn is_keypair(&self) -> bool {
        self.d.is_some()
    }

    /// Does this JWK contain only a public key?
    pub fn is_public_key(&self) -> bool {
        self.d.is_none()
    }

    /// Decode a JWK into a [`PublicKey`].
    #[cfg(feature = "arithmetic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
    pub fn to_public_key<C>(&self) -> Result<PublicKey<C>, Error>
    where
        C: Curve + JwkParameters + ProjectiveArithmetic,
        AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
        ProjectivePoint<C>: From<AffinePoint<C>>,
        Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
        UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
        UncompressedPointSize<C>: ArrayLength<u8>,
    {
        self.try_into()
    }

    /// Decode a JWK into a [`SecretKey`].
    #[cfg(feature = "arithmetic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
    pub fn to_secret_key<C>(&self) -> Result<SecretKey<C>, Error>
    where
        C: Curve + JwkParameters + ValidatePublicKey + SecretValue,
        C::Secret: Clone + Zeroize,
        FieldBytes<C>: From<C::Secret>,
        UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
        UncompressedPointSize<C>: ArrayLength<u8>,
    {
        self.try_into()
    }
}

impl FromStr for JwkEcKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        serde_json::from_str(s).map_err(|_| Error)
    }
}

impl ToString for JwkEcKey {
    fn to_string(&self) -> String {
        serde_json::to_string(self).expect("JWK encoding error")
    }
}

impl<C> TryFrom<JwkEcKey> for EncodedPoint<C>
where
    C: Curve + JwkParameters,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(jwk: JwkEcKey) -> Result<EncodedPoint<C>, Error> {
        (&jwk).try_into()
    }
}

impl<C> TryFrom<&JwkEcKey> for EncodedPoint<C>
where
    C: Curve + JwkParameters,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(jwk: &JwkEcKey) -> Result<EncodedPoint<C>, Error> {
        if jwk.crv != C::CRV {
            return Err(Error);
        }

        let x = decode_base64url_fe::<C>(&jwk.x)?;
        let y = decode_base64url_fe::<C>(&jwk.y)?;
        Ok(EncodedPoint::from_affine_coordinates(&x, &y, false))
    }
}

impl<C> TryFrom<EncodedPoint<C>> for JwkEcKey
where
    C: Curve + JwkParameters,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(point: EncodedPoint<C>) -> Result<JwkEcKey, Error> {
        (&point).try_into()
    }
}

impl<C> TryFrom<&EncodedPoint<C>> for JwkEcKey
where
    C: Curve + JwkParameters,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(point: &EncodedPoint<C>) -> Result<JwkEcKey, Error> {
        match point.coordinates() {
            Coordinates::Uncompressed { x, y } => Ok(JwkEcKey {
                crv: C::CRV.to_owned(),
                x: encode_base64url(x),
                y: encode_base64url(y),
                d: None,
            }),
            _ => Err(Error),
        }
    }
}

impl<C> TryFrom<JwkEcKey> for SecretKey<C>
where
    C: Curve + JwkParameters + ValidatePublicKey + SecretValue,
    C::Secret: Clone + Zeroize,
    FieldBytes<C>: From<C::Secret>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(jwk: JwkEcKey) -> Result<SecretKey<C>, Error> {
        (&jwk).try_into()
    }
}

impl<C> TryFrom<&JwkEcKey> for SecretKey<C>
where
    C: Curve + JwkParameters + ValidatePublicKey + SecretValue,
    C::Secret: Clone + Zeroize,
    FieldBytes<C>: From<C::Secret>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(jwk: &JwkEcKey) -> Result<SecretKey<C>, Error> {
        if let Some(d_base64) = &jwk.d {
            let pk = EncodedPoint::<C>::try_from(jwk)?;
            let mut d_bytes = decode_base64url_fe::<C>(d_base64)?;
            let result = SecretKey::from_bytes(&d_bytes);
            d_bytes.zeroize();

            result.and_then(|secret_key| {
                C::validate_public_key(&secret_key, &pk)?;
                Ok(secret_key)
            })
        } else {
            Err(Error)
        }
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> From<SecretKey<C>> for JwkEcKey
where
    C: Curve + JwkParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>> + Zeroize,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from(sk: SecretKey<C>) -> JwkEcKey {
        (&sk).into()
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> From<&SecretKey<C>> for JwkEcKey
where
    C: Curve + JwkParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>> + Zeroize,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from(sk: &SecretKey<C>) -> JwkEcKey {
        let mut jwk = sk.public_key().to_jwk();
        let mut d = sk.to_bytes();
        jwk.d = Some(encode_base64url(&d));
        d.zeroize();
        jwk
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> TryFrom<JwkEcKey> for PublicKey<C>
where
    C: Curve + JwkParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(jwk: JwkEcKey) -> Result<PublicKey<C>, Error> {
        (&jwk).try_into()
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> TryFrom<&JwkEcKey> for PublicKey<C>
where
    C: Curve + JwkParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(jwk: &JwkEcKey) -> Result<PublicKey<C>, Error> {
        EncodedPoint::<C>::try_from(jwk).and_then(PublicKey::try_from)
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> From<PublicKey<C>> for JwkEcKey
where
    C: Curve + JwkParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from(pk: PublicKey<C>) -> JwkEcKey {
        (&pk).into()
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> From<&PublicKey<C>> for JwkEcKey
where
    C: Curve + JwkParameters + ProjectiveArithmetic,
    AffinePoint<C>: Copy + Clone + Debug + Default + FromEncodedPoint<C> + ToEncodedPoint<C>,
    ProjectivePoint<C>: From<AffinePoint<C>>,
    Scalar<C>: PrimeField<Repr = FieldBytes<C>>,
    UntaggedPointSize<C>: Add<U1> + ArrayLength<u8>,
    UncompressedPointSize<C>: ArrayLength<u8>,
{
    fn from(pk: &PublicKey<C>) -> JwkEcKey {
        pk.to_encoded_point(false)
            .try_into()
            .expect("JWK encoding error")
    }
}

impl Debug for JwkEcKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = if self.d.is_some() {
            "Some(...)"
        } else {
            "None"
        };

        // NOTE: this implementation omits the `d` private key parameter
        f.debug_struct(JWK_TYPE_NAME)
            .field("crv", &self.crv)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("d", &d)
            .finish()
    }
}

impl PartialEq for JwkEcKey {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;

        // Compare private key in constant time
        let d_eq = match &self.d {
            Some(d1) => match &other.d {
                Some(d2) => d1.as_bytes().ct_eq(d2.as_bytes()).into(),
                None => other.d.is_none(),
            },
            None => other.d.is_none(),
        };

        self.crv == other.crv && self.x == other.x && self.y == other.y && d_eq
    }
}

impl Eq for JwkEcKey {}

impl Drop for JwkEcKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Zeroize for JwkEcKey {
    fn zeroize(&mut self) {
        if let Some(d) = &mut self.d {
            d.zeroize();
        }
    }
}

impl<'de> Deserialize<'de> for JwkEcKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        /// Field positions
        enum Field {
            Kty,
            Crv,
            X,
            Y,
            D,
        }

        /// Field visitor
        struct FieldVisitor;

        impl<'de> de::Visitor<'de> for FieldVisitor {
            type Value = Field;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Formatter::write_str(formatter, "field identifier")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    0 => Ok(Field::Kty),
                    1 => Ok(Field::Crv),
                    2 => Ok(Field::X),
                    3 => Ok(Field::Y),
                    4 => Ok(Field::D),
                    _ => Err(de::Error::invalid_value(
                        de::Unexpected::Unsigned(value),
                        &"field index 0 <= i < 5",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_bytes(value.as_bytes())
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    b"kty" => Ok(Field::Kty),
                    b"crv" => Ok(Field::Crv),
                    b"x" => Ok(Field::X),
                    b"y" => Ok(Field::Y),
                    b"d" => Ok(Field::D),
                    _ => Err(de::Error::unknown_field(
                        &String::from_utf8_lossy(value),
                        FIELDS,
                    )),
                }
            }
        }

        impl<'de> Deserialize<'de> for Field {
            #[inline]
            fn deserialize<D>(__deserializer: D) -> Result<Self, D::Error>
            where
                D: de::Deserializer<'de>,
            {
                de::Deserializer::deserialize_identifier(__deserializer, FieldVisitor)
            }
        }

        struct Visitor<'de> {
            marker: PhantomData<JwkEcKey>,
            lifetime: PhantomData<&'de ()>,
        }

        impl<'de> de::Visitor<'de> for Visitor<'de> {
            type Value = JwkEcKey;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Formatter::write_str(formatter, "struct JwkEcKey")
            }

            #[inline]
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let kty = de::SeqAccess::next_element::<String>(&mut seq)?
                    .ok_or_else(|| de::Error::invalid_length(0, &DE_ERROR_MSG))?;

                if kty != EC_KTY {
                    return Err(de::Error::custom(format!("unsupported JWK kty: {:?}", kty)));
                }

                let crv = de::SeqAccess::next_element::<String>(&mut seq)?
                    .ok_or_else(|| de::Error::invalid_length(1, &DE_ERROR_MSG))?;

                let x = de::SeqAccess::next_element::<String>(&mut seq)?
                    .ok_or_else(|| de::Error::invalid_length(2, &DE_ERROR_MSG))?;

                let y = de::SeqAccess::next_element::<String>(&mut seq)?
                    .ok_or_else(|| de::Error::invalid_length(3, &DE_ERROR_MSG))?;

                let d = de::SeqAccess::next_element::<Option<String>>(&mut seq)?
                    .ok_or_else(|| de::Error::invalid_length(4, &DE_ERROR_MSG))?;

                Ok(JwkEcKey { crv, x, y, d })
            }

            #[inline]
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut kty: Option<String> = None;
                let mut crv: Option<String> = None;
                let mut x: Option<String> = None;
                let mut y: Option<String> = None;
                let mut d: Option<String> = None;

                while let Some(key) = de::MapAccess::next_key::<Field>(&mut map)? {
                    match key {
                        Field::Kty => {
                            if kty.is_none() {
                                kty = Some(de::MapAccess::next_value::<String>(&mut map)?);
                            } else {
                                return Err(de::Error::duplicate_field(FIELDS[0]));
                            }
                        }
                        Field::Crv => {
                            if crv.is_none() {
                                crv = Some(de::MapAccess::next_value::<String>(&mut map)?);
                            } else {
                                return Err(de::Error::duplicate_field(FIELDS[1]));
                            }
                        }
                        Field::X => {
                            if x.is_none() {
                                x = Some(de::MapAccess::next_value::<String>(&mut map)?);
                            } else {
                                return Err(de::Error::duplicate_field(FIELDS[2]));
                            }
                        }
                        Field::Y => {
                            if y.is_none() {
                                y = Some(de::MapAccess::next_value::<String>(&mut map)?);
                            } else {
                                return Err(de::Error::duplicate_field(FIELDS[3]));
                            }
                        }
                        Field::D => {
                            if d.is_none() {
                                d = de::MapAccess::next_value::<Option<String>>(&mut map)?;
                            } else {
                                return Err(de::Error::duplicate_field(FIELDS[4]));
                            }
                        }
                    }
                }

                let kty = kty.ok_or_else(|| de::Error::missing_field("kty"))?;

                if kty != EC_KTY {
                    return Err(de::Error::custom(format!("unsupported JWK kty: {}", kty)));
                }

                let crv = crv.ok_or_else(|| de::Error::missing_field("crv"))?;
                let x = x.ok_or_else(|| de::Error::missing_field("x"))?;
                let y = y.ok_or_else(|| de::Error::missing_field("y"))?;

                Ok(JwkEcKey { crv, x, y, d })
            }
        }

        de::Deserializer::deserialize_struct(
            deserializer,
            JWK_TYPE_NAME,
            FIELDS,
            Visitor {
                marker: PhantomData::<JwkEcKey>,
                lifetime: PhantomData,
            },
        )
    }
}

impl Serialize for JwkEcKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use ser::SerializeStruct;

        let mut state = serializer.serialize_struct(JWK_TYPE_NAME, 5)?;

        for (i, field) in [EC_KTY, &self.crv, &self.x, &self.y].iter().enumerate() {
            state.serialize_field(FIELDS[i], field)?;
        }

        if let Some(d) = &self.d {
            state.serialize_field("d", d)?;
        }

        ser::SerializeStruct::end(state)
    }
}

/// Decode a Base64url-encoded field element
fn decode_base64url_fe<C: Curve>(s: &str) -> Result<FieldBytes<C>, Error> {
    let mut bytes = Zeroizing::new(s.as_bytes().to_vec());

    // Translate Base64url to traditional Base64
    // TODO(tarcieri): constant time implementation (in `b64ct` crate?)
    for byte in bytes.iter_mut() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => (),
            b'-' => *byte = b'+',
            b'_' => *byte = b'/',
            _ => return Err(Error),
        }
    }

    let s = str::from_utf8(&bytes).map_err(|_| Error)?;
    let mut result = FieldBytes::<C>::default();
    b64ct::decode(s, &mut result).map_err(|_| Error)?;
    Ok(result)
}

/// Encode a field element as Base64url
fn encode_base64url(bytes: &[u8]) -> String {
    let mut b64 = b64ct::encode_string(&bytes).into_bytes();

    // Translate traditional Base64 to Base64url
    // TODO(tarcieri): constant time implementation (in `b64ct` crate?)
    for byte in b64.iter_mut() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => (),
            b'+' => *byte = b'-',
            b'/' => *byte = b'_',
            _ => unreachable!(), // would be a bug in `b64ct`
        }
    }

    String::from_utf8(b64).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "dev")]
    use crate::dev::MockCurve;

    /// Example private key. From RFC 7518 Appendix C:
    /// <https://tools.ietf.org/html/rfc7518#appendix-C>
    const JWK_PRIVATE_KEY: &str = r#"
        {
          "kty":"EC",
          "crv":"P-256",
          "x":"gI0GAILBdu7T53akrFmMyGcsF3n5dO7MmwNBHKW5SV0",
          "y":"SLW_xSffzlPWrHEVI30DHM_4egVwt3NQqeUD7nMFpps",
          "d":"0_NxaRPUMQoAJt50Gz8YiTr8gRTwyEaCumd-MToTmIo"
        }
    "#;

    /// Example public key.
    const JWK_PUBLIC_KEY: &str = r#"
        {
          "kty":"EC",
          "crv":"P-256",
          "x":"gI0GAILBdu7T53akrFmMyGcsF3n5dO7MmwNBHKW5SV0",
          "y":"SLW_xSffzlPWrHEVI30DHM_4egVwt3NQqeUD7nMFpps"
        }
    "#;

    /// Example unsupported JWK (RSA key)
    const UNSUPPORTED_JWK: &str = r#"
        {
          "kty":"RSA",
          "kid":"cc34c0a0-bd5a-4a3c-a50d-a2a7db7643df",
          "use":"sig",
          "n":"pjdss8ZaDfEH6K6U7GeW2nxDqR4IP049fk1fK0lndimbMMVBdPv_hSpm8T8EtBDxrUdi1OHZfMhUixGaut-3nQ4GG9nM249oxhCtxqqNvEXrmQRGqczyLxuh-fKn9Fg--hS9UpazHpfVAFnB5aCfXoNhPuI8oByyFKMKaOVgHNqP5NBEqabiLftZD3W_lsFCPGuzr4Vp0YS7zS2hDYScC2oOMu4rGU1LcMZf39p3153Cq7bS2Xh6Y-vw5pwzFYZdjQxDn8x8BG3fJ6j8TGLXQsbKH1218_HcUJRvMwdpbUQG5nvA2GXVqLqdwp054Lzk9_B_f1lVrmOKuHjTNHq48w",
          "e":"AQAB",
          "d":"ksDmucdMJXkFGZxiomNHnroOZxe8AmDLDGO1vhs-POa5PZM7mtUPonxwjVmthmpbZzla-kg55OFfO7YcXhg-Hm2OWTKwm73_rLh3JavaHjvBqsVKuorX3V3RYkSro6HyYIzFJ1Ek7sLxbjDRcDOj4ievSX0oN9l-JZhaDYlPlci5uJsoqro_YrE0PRRWVhtGynd-_aWgQv1YzkfZuMD-hJtDi1Im2humOWxA4eZrFs9eG-whXcOvaSwO4sSGbS99ecQZHM2TcdXeAs1PvjVgQ_dKnZlGN3lTWoWfQP55Z7Tgt8Nf1q4ZAKd-NlMe-7iqCFfsnFwXjSiaOa2CRGZn-Q",
          "p":"4A5nU4ahEww7B65yuzmGeCUUi8ikWzv1C81pSyUKvKzu8CX41hp9J6oRaLGesKImYiuVQK47FhZ--wwfpRwHvSxtNU9qXb8ewo-BvadyO1eVrIk4tNV543QlSe7pQAoJGkxCia5rfznAE3InKF4JvIlchyqs0RQ8wx7lULqwnn0",
          "q":"ven83GM6SfrmO-TBHbjTk6JhP_3CMsIvmSdo4KrbQNvp4vHO3w1_0zJ3URkmkYGhz2tgPlfd7v1l2I6QkIh4Bumdj6FyFZEBpxjE4MpfdNVcNINvVj87cLyTRmIcaGxmfylY7QErP8GFA-k4UoH_eQmGKGK44TRzYj5hZYGWIC8",
          "dp":"lmmU_AG5SGxBhJqb8wxfNXDPJjf__i92BgJT2Vp4pskBbr5PGoyV0HbfUQVMnw977RONEurkR6O6gxZUeCclGt4kQlGZ-m0_XSWx13v9t9DIbheAtgVJ2mQyVDvK4m7aRYlEceFh0PsX8vYDS5o1txgPwb3oXkPTtrmbAGMUBpE",
          "dq":"mxRTU3QDyR2EnCv0Nl0TCF90oliJGAHR9HJmBe__EjuCBbwHfcT8OG3hWOv8vpzokQPRl5cQt3NckzX3fs6xlJN4Ai2Hh2zduKFVQ2p-AF2p6Yfahscjtq-GY9cB85NxLy2IXCC0PF--Sq9LOrTE9QV988SJy_yUrAjcZ5MmECk",
          "qi":"ldHXIrEmMZVaNwGzDF9WG8sHj2mOZmQpw9yrjLK9hAsmsNr5LTyqWAqJIYZSwPTYWhY4nu2O0EY9G9uYiqewXfCKw_UngrJt8Xwfq1Zruz0YY869zPN4GiE9-9rzdZB33RBw8kIOquY3MK74FMwCihYx_LiU2YTHkaoJ3ncvtvg"
        }
    "#;

    #[test]
    fn parse_private_key() {
        let jwk = JwkEcKey::from_str(JWK_PRIVATE_KEY).unwrap();
        assert_eq!(jwk.crv, "P-256");
        assert_eq!(jwk.x, "gI0GAILBdu7T53akrFmMyGcsF3n5dO7MmwNBHKW5SV0");
        assert_eq!(jwk.y, "SLW_xSffzlPWrHEVI30DHM_4egVwt3NQqeUD7nMFpps");
        assert_eq!(
            jwk.d.as_ref().unwrap(),
            "0_NxaRPUMQoAJt50Gz8YiTr8gRTwyEaCumd-MToTmIo"
        );
    }

    #[test]
    fn parse_public_key() {
        let jwk = JwkEcKey::from_str(JWK_PUBLIC_KEY).unwrap();
        assert_eq!(jwk.crv, "P-256");
        assert_eq!(jwk.x, "gI0GAILBdu7T53akrFmMyGcsF3n5dO7MmwNBHKW5SV0");
        assert_eq!(jwk.y, "SLW_xSffzlPWrHEVI30DHM_4egVwt3NQqeUD7nMFpps");
        assert_eq!(jwk.d, None);
    }

    #[test]
    fn parse_unsupported() {
        assert_eq!(JwkEcKey::from_str(UNSUPPORTED_JWK), Err(Error));
    }

    #[test]
    fn serialize_private_key() {
        let actual = JwkEcKey::from_str(JWK_PRIVATE_KEY).unwrap().to_string();
        let expected: String = JWK_PRIVATE_KEY.split_whitespace().collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn serialize_public_key() {
        let actual = JwkEcKey::from_str(JWK_PUBLIC_KEY).unwrap().to_string();
        let expected: String = JWK_PUBLIC_KEY.split_whitespace().collect();
        assert_eq!(actual, expected);
    }

    #[cfg(feature = "dev")]
    #[test]
    fn jwk_into_encoded_point() {
        let jwk = JwkEcKey::from_str(JWK_PUBLIC_KEY).unwrap();
        let point = EncodedPoint::<MockCurve>::try_from(&jwk).unwrap();
        let (x, y) = match point.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            other => panic!("unexpected coordinates: {:?}", other),
        };

        assert_eq!(&decode_base64url_fe::<MockCurve>(&jwk.x).unwrap(), x);
        assert_eq!(&decode_base64url_fe::<MockCurve>(&jwk.y).unwrap(), y);
    }

    #[cfg(feature = "dev")]
    #[test]
    fn encoded_point_into_jwk() {
        let jwk = JwkEcKey::from_str(JWK_PUBLIC_KEY).unwrap();
        let point = EncodedPoint::<MockCurve>::try_from(&jwk).unwrap();
        let jwk2 = JwkEcKey::try_from(point).unwrap();
        assert_eq!(jwk, jwk2);
    }
}
