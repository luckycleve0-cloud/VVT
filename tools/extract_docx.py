import sys
from docx import Document
from docx.table import Table
from docx.text.paragraph import Paragraph

path = r"C:\Users\kot22\Downloads\SCP_Rules_Fixed.docx"
doc = Document(path)

def iter_block_items(parent):
    from docx.oxml.ns import qn
    body = parent.element.body
    for child in body.iterchildren():
        if child.tag == qn('w:p'):
            yield Paragraph(child, parent)
        elif child.tag == qn('w:tbl'):
            yield Table(child, parent)

out = []
for block in iter_block_items(doc):
    if isinstance(block, Paragraph):
        text = block.text.strip()
        if text:
            style = block.style.name if block.style is not None else ""
            prefix = ""
            if style.startswith("Heading"):
                prefix = "#" * int(style.split()[-1]) + " "
            out.append(prefix + text)
    else:
        out.append("[TABLE]")
        for row in block.rows:
            cells = [c.text.strip().replace("\n", " / ") for c in row.cells]
            out.append(" | ".join(cells))
        out.append("[/TABLE]")

text = "\n".join(out)
with open("scp_rules_extracted.txt", "w", encoding="utf-8") as f:
    f.write(text)
print(f"paragraphs/tables: {len(out)}, chars: {len(text)}")
