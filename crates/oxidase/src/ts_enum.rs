use oxc_allocator::Vec;
use std::fmt;
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug)]
pub struct TsEnumCase<'a> {
    pub identifier: Option<&'a str>,
    pub quoted_name: &'a str,
    pub has_value: bool,
}

#[derive(Debug)]
pub struct TsEnum<'a> {
    pub name: &'a str,
    pub cases: Vec<'a, TsEnumCase<'a>>,
    // true if this enum name has appeared before.
    pub is_secondary: bool,
}

/// ```typescript
/// enum A {
//   X,
//   "!X",
//   Y = 1,
//   "!Y"
// }
/// ```
///
/// ```js
/// var A; (function (A) {
//   var X = 0; A[A["X"] = X] = "X";
//   A[A["!X"] = X + 1] = "!X";
//   var Y = 1; A[A["Y"] = Y] = "Y";
//   A[A["!Y"] = Y + 1] = "!Y";
// })(A || (A = {}));
/// ```
impl<'a> TsEnum<'a> {
    fn last_two(&self) -> Option<(Option<&TsEnumCase<'a>>, &TsEnumCase<'a>)> {
        Some(match self.cases.as_slice() {
            [.., case_before, case] => (Some(case_before), case),
            [case] => (None, case),
            [] => return None,
        })
    }

    fn write_auto_value(
        &self,
        case_before: Option<&TsEnumCase<'_>>,
        out: &mut impl fmt::Write,
    ) -> fmt::Result {
        if let Some(case_before) = case_before {
            if let Some(identifier_of_case_before) = case_before.identifier {
                out.write_fmt(format_args!(" = {} + 1", identifier_of_case_before))?;
            } else {
                out.write_fmt(format_args!(" = this[{}] + 1", case_before.quoted_name))?;
            }
        } else {
            out.write_str(" = 0")?;
        }
        Ok(())
    }

    /// Code before the value (if any), replacing `Foo (=)`
    pub fn current_case_head(&self, out: &mut impl fmt::Write) -> Result<(), fmt::Error> {
        let Some((case_before, case)) = self.last_two() else {
            return Ok(());
        };
        if let Some(identifier) = case.identifier {
            out.write_fmt(format_args!("var {}", identifier))?;
            if !case.has_value {
                self.write_auto_value(case_before, out)?;
            };
        } else {
            out.write_fmt(format_args!("this[this[{}]", case.quoted_name))?;
            if !case.has_value {
                self.write_auto_value(case_before, out)?;
            }
        }
        Ok(())
    }
    /// Code after the value (if any), replacing `,`
    pub fn current_case_tail(&self, out: &mut impl fmt::Write) -> Result<(), fmt::Error> {
        let Some((_case_before, case)) = self.last_two() else {
            return Ok(());
        };
        if let Some(identifier) = case.identifier {
            out.write_fmt(format_args!(
                "; this[this[{0}] = {1}] = {0};",
                case.quoted_name, identifier
            ))?;
        } else {
            out.write_fmt(format_args!("] = {};", case.quoted_name))?;
        }
        Ok(())
    }
}
