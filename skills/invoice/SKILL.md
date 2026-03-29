---
name: invoice
description: "Generate a professional invoice PDF from expense data or manual line items"
version: "1.0.0"
permissions: [file_write]
metadata:
  emoji: "🧾"
  requires:
    tools: ["expense_list", "pdf_generate"]
---

# Invoice Generator

Create a professional invoice document from expense records or manually specified items.

## Workflow

### Step 1: Gather Invoice Details
Ask the user for or extract from context:
- Client name and address
- Invoice number (auto-generate if not provided: INV-YYYYMMDD-001)
- Due date (default: 30 days from today)
- Tax rate (default: 0%)
- Any notes or payment terms

### Step 2: Collect Line Items
Either:
- Pull from expense tracker using `expense_list` action with date filters
- Accept manual line items from the user (description, quantity, unit price)

### Step 3: Calculate Totals
- Subtotal: sum of all line items
- Tax: subtotal * tax_rate
- Total: subtotal + tax
- If cost splitting is involved, show per-person breakdown

### Step 4: Generate PDF
Use the `pdf_generate` action with style "invoice" to create the PDF.

Pass structured data:
```json
{
  "style": "invoice",
  "title": "INVOICE",
  "content": "Invoice #{number}\nDate: {date}\nDue: {due_date}\n\nBill To:\n{client_name}\n{client_address}\n\nItems:\n{item_table}\n\nSubtotal: {subtotal}\nTax ({rate}%): {tax}\nTotal: {total}\n\nNotes: {notes}",
  "filename": "invoice_{number}.pdf"
}
```

### Step 5: Present Result
Show the invoice summary to the user and provide the PDF file path/link.

## Output Format

```
Invoice #{number} generated successfully!

Client: {client_name}
Items: {count} line items
Subtotal: {currency} {subtotal}
Tax: {currency} {tax}
Total: {currency} {total}
Due: {due_date}

PDF saved to: {filename}
```
