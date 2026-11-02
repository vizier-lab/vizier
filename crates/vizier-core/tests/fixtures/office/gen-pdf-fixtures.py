"""Generates the PDF samples in this folder, read by src/agents/tools/office/fixture_tests.rs.

Written with reportlab and pypdf, not by this crate's own PDF writer, so the reader is checked
against files this crate did not produce. Output is committed; run this only to change a sample.

    pip install reportlab pypdf pillow cryptography
    python3 gen-pdf-fixtures.py [out_dir]
"""
import io, os, sys
from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import getSampleStyleSheet
from reportlab.platypus import SimpleDocTemplate, Paragraph, Table, TableStyle, ListFlowable, ListItem, PageBreak
from reportlab.pdfgen import canvas
from reportlab.lib.utils import ImageReader
from PIL import Image, ImageDraw
from pypdf import PdfReader, PdfWriter

out = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))
os.makedirs(out, exist_ok=True)
styles = getSampleStyleSheet()

def build(path, story):
    # invariant=1 keeps the output byte-stable between runs (no creation date or random id)
    SimpleDocTemplate(path, pagesize=A4, invariant=1, title="Sample").build(story)

# ---- table.pdf: heading, paragraph, list, table (1 page) ----
t = Table([["Name", "Qty"], ["Widget", "3"], ["Gadget", "12"]])
t.setStyle(TableStyle([("GRID", (0, 0), (-1, -1), 0.5, colors.black), ("BACKGROUND", (0, 0), (-1, 0), colors.lightgrey)]))
build(os.path.join(out, "table.pdf"), [
    Paragraph("Quarterly Report", styles["Heading1"]),
    Paragraph("Sales were steady across all regions.", styles["Normal"]),
    ListFlowable([ListItem(Paragraph("First item", styles["Normal"])), ListItem(Paragraph("Second item", styles["Normal"]))], bulletType="bullet"),
    t,
])

# ---- long.pdf: 30 pages, each opens with a unique PAGE-n marker ----
story = []
for n in range(1, 31):
    story += [Paragraph(f"PAGE-{n}", styles["Heading1"]), Paragraph(f"Body text of page {n}. " * 12, styles["Normal"]), PageBreak()]
build(os.path.join(out, "long.pdf"), story[:-1])

# ---- scanned.pdf: one page that is only a raster image, no text layer ----
img = Image.new("RGB", (600, 300), "white")
ImageDraw.Draw(img).text((40, 120), "SCANNED PAGE (image only)", fill="black")
buf = io.BytesIO(); img.save(buf, "PNG"); buf.seek(0)
c = canvas.Canvas(os.path.join(out, "scanned.pdf"), pagesize=A4, invariant=1)
c.drawImage(ImageReader(buf), 50, 400, width=500, height=250)
c.save()

# ---- locked.pdf: encrypted, user password "vizier", content "Locked Report" ----
src = io.BytesIO()
SimpleDocTemplate(src, pagesize=A4, invariant=1).build([Paragraph("Locked Report", styles["Heading1"]), Paragraph("Secret figures inside.", styles["Normal"])])
w = PdfWriter(clone_from=PdfReader(src))
w.encrypt("vizier")
with open(os.path.join(out, "locked.pdf"), "wb") as f:
    w.write(f)

# ---- corrupt.pdf: the header and the first bytes of a real file, nothing the library can recover a page from ----
good = open(os.path.join(out, "table.pdf"), "rb").read()
open(os.path.join(out, "corrupt.pdf"), "wb").write(good[:120])
