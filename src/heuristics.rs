/// Keywords that indicate product availability / purchase intent.
pub static AVAILABILITY_HEURISTICS: &[&str] = &[
    "In den Warenkorb", "In den Einkaufswagen", "Jetzt kaufen", "Kaufen", "Bestellen",
    "Add to Cart", "Add to Basket", "Buy Now", "Buy", "Order Now", "Order", "In Stock", "Verfügbar", "Lieferbar"
];

/// Keywords that indicate product detail content.
pub static DETAILS_HEURISTICS: &[&str] = &[
    "Produktbeschreibung", "Produktdetails", "Produktinformationen", "Technische Daten", "Spezifikationen",
    "Product Description", "Product Details", "Product Information", "Technical Specifications", "Specs"
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

