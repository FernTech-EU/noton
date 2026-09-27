#!/usr/bin/env python3
"""Acceptance probe for Teksilo's new table accessibility, in Noton's real UI.

Requires the updated local Teksilo AND text-typeset sources. See
docs/teksilo-integration.md for the committed-pin limitation.
"""
import pathlib
import tempfile

from mcp_smoke import Client

CELL_TEXT = "Cellule — café"


def assert_table(client):
    nodes = client.nodes()
    by_id = {node["id"]: node for node in nodes}
    tables = [node for node in nodes if node["role"] == "Table"]
    assert len(tables) == 1, "Expected one accessible table; check dependency overrides"
    rows = [by_id[node_id] for node_id in tables[0]["children"]]
    assert len(rows) == 3 and all(row["role"] == "Row" for row in rows), rows
    cells = []
    for row in rows:
        row_cells = [by_id[node_id] for node_id in row["children"]]
        assert len(row_cells) == 3, row_cells
        assert all(cell["role"] == "Cell" for cell in row_cells), row_cells
        cells.extend(row_cells)
    for cell in cells:
        assert "bounds" in cell, f"Accessible cell has no bounds: {cell}"
        bounds = cell["bounds"]
        assert bounds["width"] > 0 and bounds["height"] > 0, cell
        containers = [by_id[node_id] for node_id in cell["children"]]
        assert any(node["role"] == "Label" and "text" in node for node in containers), cell
    # The text must live under a cell, not merely somewhere else in the editor.
    cell_text = [
        by_id[node_id].get("text", {}).get("document_text", "")
        for cell in cells
        for node_id in cell["children"]
    ]
    assert CELL_TEXT in cell_text, cell_text
    return {node["id"] for node in [tables[0], *rows, *cells]}


def run():
    with tempfile.TemporaryDirectory() as directory:
        path = pathlib.Path(directory) / "table.noton"
        client = Client(path)
        try:
            for label in ["New folder", "New note"]:
                client.click(label)
            client.insert("Text block")
            editor = next(node["id"] for node in client.nodes() if node["role"] == "MultilineTextInput")
            client.execute("FocusNode", node=editor)
            client.click("Table")
            client.click("Insert 3 × 3 table", "MenuItem")
            client.execute("TypeText", node=editor, text=CELL_TEXT)
            identities = assert_table(client)
            # An ordinary cell edit must not replace the structural nodes.
            client.execute("TypeText", node=editor, text="!")
            # Remove only the appended character via the editor's native key path.
            client.execute("InjectKey", key="Backspace")
            assert assert_table(client) == identities
            client.click("Reading", "RadioButton")
            assert_table(client)
            client.execute("InjectKey", key="s", command=True)
            client.close()
            client = Client(path)
            # The reopened canvas materializes its cards on the first frame;
            # geometry is published from the next one.
            client.nodes()
            assert_table(client)
            client.click("Reading", "RadioButton")
            assert_table(client)
            client.close()
            print("MCP tables: structure, cell text, geometry, stable IDs, both views, save/reopen: passed")
        finally:
            if client.p.poll() is None:
                client.kill()


if __name__ == "__main__":
    run()
