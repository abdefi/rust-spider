/// Keywords that indicate product availability / purchase intent.
pub static AVAILABILITY_HEURISTICS: &[&str] = &[
    // German
    "in den warenkorb", "in den einkaufswagen", "jetzt kaufen", "bestellen",
    "verfügbar", "lieferbar", "vorrätig",
    // English
    "add to cart", "add to basket", "buy now", "order now",
    "in stock", "purchase", "add to bag",
    // Auction-specific
    "place bid", "bid now", "register to bid", "view lot",
    "price realized", "sold for", "starting bid",
];

/// Keywords that indicate product detail content.
pub static DETAILS_HEURISTICS: &[&str] = &[
    // German
    "produktbeschreibung", "produktdetails", "produktinformationen",
    "technische daten", "spezifikationen", "maße", "beschribung",
    // English
    "product description", "product details", "product information",
    "technical specifications",
    // Auction-specific
    "provenance", "condition report",
    "lot details", "estimate",
    "dimensions", "signed and dated",
];

/// Returns `true` if the HTML contains at least one availability keyword
/// **and** at least one detail keyword (case-insensitive).
pub fn is_product_page(html: &str) -> bool {
    let lower = html.to_lowercase();

    let has_availability = AVAILABILITY_HEURISTICS
        .iter()
        .any(|kw| lower.contains(kw));

    let has_details = DETAILS_HEURISTICS
        .iter()
        .any(|kw| lower.contains(kw));


    has_availability && has_details
}
