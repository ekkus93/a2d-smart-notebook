-- TODO 5.5: a generated page (Smart Page or Page Set member) needs to remember which Asset its
-- generated PDF was committed as, so a later "reprint existing PDF" (TODO 10.4) or asset-attach
-- step can find it. Nullable: scanned/imported pages never had a generated PDF at all.
ALTER TABLE pages ADD COLUMN generated_pdf_asset_id TEXT REFERENCES assets (id);
