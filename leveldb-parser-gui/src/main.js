const { getCurrentWebview } = window.__TAURI__.webview
const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event

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
        { field: "Seq" },
        { field: "State" },
        { field: "Key" },
        { field: "Value" }
    ],
    rowData: [],
}


const myGridElement = document.querySelector('#myGrid')
const gridApi = agGrid.createGrid(myGridElement, gridOptions)

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
            // remove hex byte sequences
            result.push(current.replace(/\\x[0-9A-Fa-f]{2}/g, ''))
            current = ''
        } else {
            current += char
        }
    }
    // remove hex byte sequences from the last field
    result.push(current.replace(/\\x[0-9A-Fa-f]{2}/g, ''))
    return result
}

// -----------------------------------------------------------------------------
listen('ldb_csv', e => {
    const csv = e.payload
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            obj[header.charAt(0).toUpperCase() + header.slice(1)] = values[idx]
        })
        return obj
    })

    gridApi.setGridOption('rowData', rowData)
})
