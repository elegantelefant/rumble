# ABOUTME: Fills bracketed placeholders in a drafted document with the form's own
# ABOUTME: field values, generalised from labels/aliases rather than hardcoded per field.

import re
from datetime import date

from models.generated import DraftField

BRACKET_RE = re.compile(r"\[([^\[\]]{1,80})\]")


def _normalise(text: str) -> str:
    """Lowercases and strips a possessive "'s"/"'s" as a unit before
    dropping any remaining apostrophe, so "Employee's Name" lines up with
    the label "Employee Name" -- stripping only the apostrophe character
    would leave "employees name", one letter short of matching."""
    text = text.strip().lower()
    text = re.sub(r"[’']s\b", "", text)
    text = text.replace("’", "").replace("'", "")
    return re.sub(r"\s+", " ", text).strip()


def _prose_date(iso_value: str) -> str:
    """"2026-09-19" -> "19 September 2026". Falls back to the raw value if it
    isn't a plain ISO date -- insertion should never raise on an odd input."""
    try:
        y, m, d = (int(x) for x in str(iso_value).split("-"))
        dt = date(y, m, d)
        return f"{dt.day} {dt.strftime('%B')} {dt.year}"
    except (ValueError, TypeError):
        return str(iso_value)


def _format_value(field: DraftField) -> str:
    if field.type == "date":
        return _prose_date(field.value)
    return field.value


def _field_labels(field: DraftField) -> list[str]:
    """A field's label plus every alias -- deliberately treated identically
    everywhere a label is matched, per the field's own `aliases` list."""
    return [field.label, *(field.aliases or [])]


def _label_set(field: DraftField) -> set[str]:
    return {_normalise(label) for label in _field_labels(field)}


def _value_literal_candidates(field: DraftField) -> set[str]:
    """Exact strings meaning "this bracket just echoes the provided value
    itself", e.g. value "Tester" -> {"tester", "tester name", "testers name"}.
    The "name" suffix only applies when the field's own label is name-like."""
    v = _normalise(field.value)
    candidates = {v}
    if "name" in _normalise(field.label):
        candidates |= {f"{v} name", f"{v}s name", f"{v} full name"}
    return candidates


def _qualified_label_candidates(value_field: DraftField, label_field: DraftField) -> set[str]:
    """"<value_field's value> <label_field's label/alias>" -- e.g. value_field
    is employeeName ("Tester"), label_field is position ("Position") ->
    {"tester position", "testers position"}. Generalises "[Tester Position]"
    to any pair of fields, not just employee name + position."""
    v = _normalise(value_field.value)
    out = set()
    for label in _label_set(label_field):
        out |= {f"{v} {label}", f"{v}s {label}"}
    return out


def classify_bracket(inner: str, fields: list[DraftField]) -> DraftField | None:
    """Return the field a bracket should be substituted with, or None to leave it.

    Checked in order:
      1. The bracket exactly equals a field's label or one of its aliases.
      3. "[Label: anything]" -- the label alone decides; whatever follows the
         colon is discarded in favour of the field's own value.
      0. The bracket is just the field's own provided value (optionally with
         a "name" suffix, for name-like fields).
      2. The bracket is "<another field's value> <this field's label/alias>".
    Every rule is an EXACT match on the normalised bracket text against a
    precomputed candidate set, never a substring test -- so "[Tester's
    Address]" (employee name "Tester", no address field) never collides
    with anything: "testers address" isn't itself a candidate, even though
    "tester" is a provided value and would be if checked by substring.
    """
    norm = _normalise(inner)

    for field in fields:
        if norm in _label_set(field):
            return field

    if ":" in norm:
        label_part = norm.split(":", 1)[0].strip()
        for field in fields:
            if label_part in _label_set(field):
                return field

    for field in fields:
        if norm in _value_literal_candidates(field):
            return field

    for value_field in fields:
        for label_field in fields:
            if value_field is label_field:
                continue
            if norm in _qualified_label_candidates(value_field, label_field):
                return label_field

    return None


def apply(draft: str, fields: list[DraftField]) -> tuple[str, list[dict]]:
    """Returns (new_draft, substitutions_made). Values are inserted verbatim,
    exactly as given, except date-typed fields which are rendered as a prose
    date (e.g. "19 September 2026")."""
    substitutions: list[dict] = []

    def repl(match: re.Match) -> str:
        full, inner = match.group(0), match.group(1)
        field = classify_bracket(inner, fields)
        if field is None:
            return full
        inserted = _format_value(field)
        substitutions.append({"placeholder": full, "key": field.key, "inserted": inserted})
        return inserted

    return BRACKET_RE.sub(repl, draft), substitutions


# --- Presence check, for the retry-once decision in routes/jobs.py ---

def _date_variants(iso_value: str) -> set[str]:
    """A handful of common prose/numeric renderings of an ISO date, lowercased."""
    try:
        y, m, d = (int(x) for x in str(iso_value).split("-"))
        dt = date(y, m, d)
    except (ValueError, TypeError):
        return {str(iso_value).lower()}
    return {
        str(iso_value).lower(),
        dt.strftime("%B %d, %Y").lower(),
        dt.strftime("%B %d, %Y").replace(" 0", " ").lower(),
        dt.strftime("%b %d, %Y").lower(),
        dt.strftime("%m/%d/%Y").lower(),
        dt.strftime("%d %B %Y").lower(),
        f"{dt.day} {dt.strftime('%B')} {dt.year}".lower(),
    }


def _looks_like_amount(value: str) -> bool:
    stripped = re.sub(r"[$,\s]", "", value)
    return bool(stripped) and stripped.replace(".", "", 1).isdigit()


def _amount_present(draft: str, value: str) -> bool:
    digits = re.sub(r"\D", "", value)
    if not digits:
        return False
    return any(re.sub(r"\D", "", run) == digits for run in re.findall(r"[\d,]+", draft) if re.sub(r"\D", "", run))


_STOPWORDS = {"the", "a", "an", "of", "and", "to", "for", "in", "on", "with", "is", "this", "that"}


def _freetext_present(draft: str, value: str) -> bool:
    """A paraphrase counts, not just a verbatim match: at least half of the
    value's significant words (longer than 2 letters, not a stopword) must
    appear somewhere in the draft. A verbatim restatement still passes --
    this only widens what counts, it never narrows it."""
    lowered = draft.lower()
    words = [w for w in re.findall(r"[a-z0-9]+", value.lower()) if len(w) > 2 and w not in _STOPWORDS]
    if not words:
        return value.strip().lower() in lowered
    hits = sum(1 for w in words if w in lowered)
    return hits >= max(1, (len(words) + 1) // 2)


def all_present(draft: str, fields: list[DraftField]) -> dict[str, bool]:
    """Per-field key -> whether that value made it into the draft, in some
    recognisable form. Dates are normalised across common renderings;
    amounts compare digits only (so "$100,000" matches "100000"); freetext
    fields accept a paraphrase; everything else is a literal substring check."""
    lowered = draft.lower()
    result = {}
    for field in fields:
        if field.type == "date":
            result[field.key] = any(v in lowered for v in _date_variants(field.value))
        elif field.type == "freetext":
            result[field.key] = _freetext_present(draft, field.value)
        elif _looks_like_amount(field.value):
            result[field.key] = _amount_present(draft, field.value)
        else:
            result[field.key] = _normalise(field.value) in lowered
    return result
