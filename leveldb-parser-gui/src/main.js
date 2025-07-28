const { getCurrentWebview } = window.__TAURI__.webview
const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event
const { writeText, readText } = window.__TAURI__.clipboardManager

const overlay = document.getElementById('drop-overlay')
const dropAreaWrapper = document.getElementById('drop-area-wrapper')
const contentWrapper = document.getElementById('wrapper')

const recordsButton = document.getElementById('records-button')
const manifestButton = document.getElementById('manifest-button')
const logButton = document.getElementById('log-button')

const loadingIndicator = document.getElementById('loading-indicator')
const searchContainer = document.getElementById('search-container')

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

await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === 'over') {
        overlay.classList.add('active')
    } else if (e.payload.type === 'drop') {
        overlay.classList.remove('active')
        dropAreaWrapper.style.display = 'none'
        contentWrapper.style.display = 'block'

        requestAnimationFrame(() => {
            invoke('process_dropped_files', { paths: e.payload.paths })
        })
    } else {
        overlay.classList.remove('active')
    }
})

const CUT_OFF_LEN = 300
const largeValRenderer = (params) => {
    if (!params.value) return ''

    const fullValue = params.value
    const isLarge = fullValue.length > CUT_OFF_LEN

    if (!isLarge) return fullValue

    const remainingChars = fullValue.length - CUT_OFF_LEN
    const truncated = fullValue.substring(0, CUT_OFF_LEN) + `<span style="color: red;">[+${remainingChars.toLocaleString()} Chars...]</span>`

    return truncated
}

// records-grid ----------------------------------------------------------------
const gridOptionsRecords = {
    columnDefs: [
        {
            field: "Seq",
            comparator: (valueA, valueB) => valueA - valueB,
            headerName: "Seq. #",
            flex: 0.4,
            minWidth: 40,
        },
        { field: "K", headerName: "Key", flex: 2, minWidth: 100 },
        { field: "V", headerName: "Value", flex: 5, minWidth: 100, cellRenderer: largeValRenderer },
        { field: "Cr", headerName: "CRC32", flex: 0.4, minWidth: 40 },
        { field: "St", headerName: "State", flex: 0.4, minWidth: 40 },
        { field: "BO", comparator: (valueA, valueB) => valueA - valueB, headerName: "Block Offset", flex: 0.4, minWidth: 90 },
        { field: "C", headerName: "Compressed", flex: 0.4, minWidth: 90, cellStyle: { pointerEvents: 'none' } },
        { field: "F", headerName: "File", flex: 0.4, minWidth: 80 },
    ],
    defaultColDef: {
        filter: true,
    },
    rowData: [],
    overlayLoadingTemplate: '<p style="font-weight: bold; color: orangered;">Loading...</p>',
    overlayNoRowsTemplate: '<p style="font-weight: bold; color: orangered;">No Data</p>',
    animateRows: false,
    rowBuffer: 50,
    debounceVerticalScrollbar: true,
    getRowStyle: params => {
        if (params.data && params.data.Cr && params.data.Cr.includes('failed')) {
            return { color: 'red' }
        } else if (params.data && params.data.St && params.data.St.includes('deleted')) {
            return { backgroundColor: '#f2f2f6' }
        }
        return null
    }
}

const recordsGridElem = document.querySelector('#records-grid')
const recordsGrid = agGrid.createGrid(recordsGridElem, gridOptionsRecords)

// manifest-grid ---------------------------------------------------------------
const gridOptionsManifest = {
    columnDefs: [
        { field: "Tag", flex: 0.5, minWidth: 100 },
        { field: "TagValue", headerName: "Value", flex: 5, minWidth: 200 },
        { field: "CRC", headerName: "CRC32", flex: 0.4, minWidth: 40 },
        { field: "BlockOffset", headerName: "Block Offset", flex: 0.4, minWidth: 90 },
        { field: "File", flex: 0.4, minWidth: 115 },
    ],
    defaultColDef: {
        filter: true,
    },
    rowData: [],
    overlayLoadingTemplate: '<p style="font-weight: bold; color: orangered;">Loading...</p>',
    overlayNoRowsTemplate: '<p style="font-weight: bold; color: orangered;">No Data</p>',
    animateRows: false,
    rowBuffer: 50,
    debounceVerticalScrollbar: true,
    getRowStyle: params => {
        if (params.data && params.data.CRC && params.data.CRC.includes('failed')) {
            return { color: 'red' }
        }
        return null
    }
}

const manifestGridElem = document.querySelector('#manifest-grid')
const manifestGrid = agGrid.createGrid(manifestGridElem, gridOptionsManifest)

// log-text-grid ---------------------------------------------------------------
const gridOptionsLogText = {
    columnDefs: [
        { field: "Date", flex: 0.3, minWidth: 150, sort: 'asc' },
        { field: "ThreadId", headerName: "ThreadID", flex: 0.2, minWidth: 80 },
        { field: "Msg", headerName: "Message", flex: 5, minWidth: 300 },
        { field: "File", flex: 0.2, minWidth: 80 },
    ],
    defaultColDef: {
        filter: true,
    },
    rowData: [],
    overlayLoadingTemplate: '<p style="font-weight: bold; color: orangered;">Loading...</p>',
    overlayNoRowsTemplate: '<p style="font-weight: bold; color: orangered;">No Data</p>',
    animateRows: false,
    rowBuffer: 50,
    debounceVerticalScrollbar: true,
}

const logTextGridElem = document.querySelector('#log-text-grid')
const logTextGrid = agGrid.createGrid(logTextGridElem, gridOptionsLogText)

// listener --------------------------------------------------------------------
listen('processing_started', () => {
    loadingIndicator.style.display = 'block'
})

listen('processing_finished', () => {
    loadingIndicator.style.display = 'none'
})

let isFirstLoad = true
listen('records_csv', e => {
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

    const currentRowData = recordsGrid.getGridOption('rowData') || []
    const combinedRowData = [...currentRowData, ...rowData]

    recordsGrid.setGridOption('rowData', combinedRowData)

    if (isFirstLoad) {
        showTab('records')
        isFirstLoad = false
    }
})

listen('manifest_csv', e => {
    const csv = e.payload
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            obj[header] = values[idx]
        })
        return obj
    })

    const currentRowData = manifestGrid.getGridOption('rowData') || []
    const combinedRowData = [...currentRowData, ...rowData]

    manifestGrid.setGridOption('rowData', combinedRowData)

    if (!recordsButton.classList.contains('active-tab-button')) {
        showTab('manifest')
    }
})

listen('log_text_csv', e => {
    const csv = e.payload
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            obj[header] = values[idx]
        })
        return obj
    })

    const currentRowData = logTextGrid.getGridOption('rowData') || []
    const combinedRowData = [...currentRowData, ...rowData]

    logTextGrid.setGridOption('rowData', combinedRowData)

    if (!recordsButton.classList.contains('active-tab-button') && !manifestButton.classList.contains('active-tab-button')) {
        showTab('log')
    }
})

// copy to clipboard
document.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
        e.preventDefault()
        const focusedCell = getFocusedCellValue(recordsGrid)
        await writeText(focusedCell)
    }
})

function getFocusedCellValue(gridApi) {
    const focusedCell = gridApi.getFocusedCell()
    const rowNode = gridApi.getDisplayedRowAtIndex(focusedCell.rowIndex)
    return rowNode.data[focusedCell.column.getColId()]
}

// global search (quick filter)
const filterTextBox = document.querySelector('#filter-text-box')
let searchTimeout = null

filterTextBox.addEventListener('input', function () {
    filterTextBox.classList.add('searching')

    const activeTab = document.querySelector('.active-tab-button').id
    const activeGrid = activeTab === 'records-button' ? recordsGrid : manifestGrid

    if (activeTab === 'records-button' || activeTab === 'manifest-button') {
        activeGrid.setGridOption('loading', true)

        // debounce
        if (searchTimeout) {
            clearTimeout(searchTimeout)
        }

        searchTimeout = setTimeout(() => {
            activeGrid.setGridOption('quickFilterText', this.value)
            activeGrid.setGridOption('loading', false)
            filterTextBox.classList.remove('searching')
        }, 300)
    }
})

// clear button
document.querySelector('#clear-button').addEventListener('click', () => {
    window.location.reload()
})

// tab switching
recordsButton.addEventListener('click', () => {
    showTab('records')
})

manifestButton.addEventListener('click', () => {
    showTab('manifest')
})

logButton.addEventListener('click', () => {
    showTab('log')
})

function showTab(tabId) {
    const tabs = ['records', 'manifest', 'log']
    tabs.forEach(id => {
        const el = document.getElementById(id)
        if (el) {
            el.style.display = (id === tabId) ? 'block' : 'none'
        }
    })

    document.querySelectorAll('.tab-button').forEach(button => {
        button.classList.remove('active-tab-button')
    })

    document.getElementById(`${tabId}-button`).classList.add('active-tab-button')

    // hide search if not records or manifest
    if (tabId === 'log') {
        searchContainer.style.display = 'none'
    } else {
        searchContainer.style.display = 'block'
    }
}
