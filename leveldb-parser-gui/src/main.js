const { getCurrentWebview } = window.__TAURI__.webview
const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event
const { writeText, readText } = window.__TAURI__.clipboardManager

const overlay = document.getElementById('drop-overlay')

await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === 'over') {
        overlay.classList.add('active')
    } else if (e.payload.type === 'drop') {
        overlay.classList.remove('active')
        handleDrop(e.payload.paths)
    } else {
        overlay.classList.remove('active')
    }
})

function handleDrop(file_paths) {
    invoke('process_dropped_files', { paths: file_paths })
}

// ag-grid ---------------------------------------------------------------------
const gridOptions = {
    columnDefs: [
        {
            field: "Seq",
            comparator: (valueA, valueB) => valueA - valueB,
            headerName: "Seq. #",
            flex: 0.4,
            minWidth: 40,
        },
        { field: "K", headerName: "Key", flex: 2, minWidth: 100 },
        { field: "V", headerName: "Value", flex: 5, minWidth: 100 },
        { field: "Cr", headerName: "CRC32", flex: 0.4, minWidth: 40 },
        { field: "St", headerName: "State", flex: 0.4, minWidth: 40 },
        { field: "BO", headerName: "Block Offset", flex: 0.4, minWidth: 90 },
        { field: "C", headerName: "Compressed", flex: 0.4, minWidth: 90, cellStyle: { pointerEvents: 'none' } },
        { field: "F", headerName: "File", flex: 0.4, minWidth: 80 },
    ],
    defaultColDef: {
        filter: true
    },
    rowData: [],
    overlayNoRowsTemplate: 'Drop LevelDB folder or file to parse',
    animateRows: false,
}

const myGridElement = document.querySelector('#myGrid')
const gridApi = agGrid.createGrid(myGridElement, gridOptions)

// listener --------------------------------------------------------------------
listen('ldb_csv', e => {
    const csv = e.payload
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            if (header === "C") {
                obj[header] = values[idx] === "true"
            } else {
                obj[header] = values[idx]
            }
        })
        return obj
    })
    gridApi.setGridOption('rowData', rowData)
})

// copy to clipboard
document.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
        e.preventDefault()
        const focusedCell = getFocusedCellValue(gridApi)
        await writeText(focusedCell)
    }
})

function getFocusedCellValue(gridApi) {
    const focusedCell = gridApi.getFocusedCell()
    const rowNode = gridApi.getDisplayedRowAtIndex(focusedCell.rowIndex)
    return rowNode.data[focusedCell.column.getColId()]
}

// helper ----------------------------------------------------------------------
function parseCsvLine(line) {
    const result = []
    let current = ''
    let inQuotes = false

    for (let i = 0; i < line.length; i++) {
        const char = line[i]
        if (char === '"' && line[i + 1] === '"') {
            current += '"'
            i++ // skip next quote
        } else if (char === '"') {
            inQuotes = !inQuotes
        } else if (char === ',' && !inQuotes) {
            result.push(current)
            current = ''
        } else {
            current += char
        }
    }
    result.push(current)
    return result
}