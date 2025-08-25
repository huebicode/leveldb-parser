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
    const fullValue = params.value ?? ''
    const isLarge = fullValue.length > CUT_OFF_LEN

    const container = document.createElement('span')
    container.textContent = isLarge ? fullValue.substring(0, CUT_OFF_LEN) : fullValue

    if (isLarge) {
        const remainingChars = fullValue.length - CUT_OFF_LEN
        const badge = document.createElement('span')
        badge.style.color = 'red'
        badge.textContent = ` [+${remainingChars.toLocaleString()} Chars]`
        container.appendChild(badge)
    }

    return container
}

const valuePopup = document.getElementById('value-popup')
const popupContent = document.getElementById('popup-content')

function escapeHtml(text) {
    return text.replace(/[&<>"']/g, function (m) {
        switch (m) {
            case '&': return '&amp;'
            case '<': return '&lt;'
            case '>': return '&gt;'
            case '"': return '&quot;'
            case "'": return '&#39;'
            default: return m
        }
    })
}

function showValuePopup(value) {
    const activeTabButton = document.querySelector('.active-tab-button')
    let searchTerm = ''
    if (activeTabButton) {
        const tabId = activeTabButton.id.replace('-button', '')
        const searchInput = document.getElementById(`${tabId}-search-input`)
        if (searchInput && searchInput.value.trim()) {
            searchTerm = searchInput.value.trim()
        }
    }

    // escape HTML
    let html = escapeHtml(value)
    if (searchTerm) {
        const escapedTerm = searchTerm.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
        const regex = new RegExp(escapedTerm, 'gi')
        html = html.replace(regex, match => `<mark>${match}</mark>`)
    }

    popupContent.innerHTML = html
    valuePopup.style.display = 'flex'
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
        {
            field: "V", headerName: "Value", flex: 5, minWidth: 100,
            cellRenderer: largeValRenderer,
            onCellDoubleClicked: (params) => {
                if (params.value) {
                    showValuePopup(params.value)
                }
            },
        },
        { field: "Cr", headerName: "CRC32", flex: 0.4, minWidth: 40 },
        { field: "St", headerName: "State", flex: 0.4, minWidth: 40 },
        { field: "BO", comparator: (valueA, valueB) => valueA - valueB, headerName: "Block Offset", flex: 0.4, minWidth: 90 },
        { field: "C", headerName: "Compressed", flex: 0.4, minWidth: 90, cellStyle: { pointerEvents: 'none' } },
        { field: "F", headerName: "File", flex: 0.4, minWidth: 80 },
        { field: "FP", headerName: "File Path", flex: 0.4, minWidth: 80 },
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
        { field: "FilePath", headerName: "File Path", flex: 0.4, minWidth: 115 },
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
        { field: "FilePath", headerName: "File Path", flex: 0.2, minWidth: 80 },
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

    recordsGrid.applyTransaction({ add: rowData })

    if (isFirstLoad) {
        showTab('records')
        isFirstLoad = false
    }
    updateRowCount()
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

    manifestGrid.applyTransaction({ add: rowData })

    if (!recordsButton.classList.contains('active-tab-button')) {
        showTab('manifest')
    }
    updateRowCount()
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

    logTextGrid.applyTransaction({ add: rowData })

    if (!recordsButton.classList.contains('active-tab-button') && !manifestButton.classList.contains('active-tab-button')) {
        showTab('log')
    }
    updateRowCount()
})

// value popup
document.addEventListener('mousedown', (e) => {
    if (valuePopup.style.display !== 'none' && e.target.id !== 'popup-content') {
        valuePopup.style.display = 'none'
    }
})

document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && valuePopup.style.display !== 'none') {
        valuePopup.style.display = 'none'
    }
})

// copy to clipboard
document.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'c' && valuePopup.style.display === 'none') {
        e.preventDefault()

        const activeTab = document.querySelector('.active-tab-button').id
        let activeGrid

        if (activeTab === 'records-button') {
            activeGrid = recordsGrid
        } else if (activeTab === 'manifest-button') {
            activeGrid = manifestGrid
        } else if (activeTab === 'log-button') {
            activeGrid = logTextGrid
        }

        if (activeGrid) {
            const focusedCell = getFocusedCellValue(activeGrid)
            if (focusedCell !== undefined) {
                await writeText(focusedCell)
            }
        }
    }
})

function getFocusedCellValue(gridApi) {
    const focusedCell = gridApi.getFocusedCell()
    const rowNode = gridApi.getDisplayedRowAtIndex(focusedCell.rowIndex)
    return rowNode.data[focusedCell.column.getColId()]
}

// search inputs (quick filter)
let searchTimeout = null
document.querySelectorAll('[id$="search-input"]').forEach(inputElement => {
    const prefix = inputElement.id.replace('-search-input', '')
    const inputContainer = inputElement.closest('.input-container')
    inputContainer.style.display = 'none'

    const clearButton = document.getElementById(`${prefix}-clear-button`)
    clearButton.style.display = 'none'

    let gridApi
    switch (prefix) {
        case 'records':
            gridApi = recordsGrid
            break
        case 'manifest':
            gridApi = manifestGrid
            break
        case 'log':
            gridApi = logTextGrid
            break
    }

    inputElement.addEventListener('input', function () {
        gridApi.setGridOption('loading', true)

        if (clearButton) {
            clearButton.style.display = this.value ? 'inline-block' : 'none'
        }

        // debounce
        if (searchTimeout) {
            clearTimeout(searchTimeout)
        }

        searchTimeout = setTimeout(() => {
            gridApi.setGridOption('quickFilterText', this.value)
            gridApi.setGridOption('loading', false)
            updateRowCount()
        }, 300)
    })

    if (clearButton) {
        clearButton.addEventListener('click', () => {
            inputElement.value = ''
            clearButton.style.display = 'none'
            inputElement.dispatchEvent(new Event('input'))
            inputElement.focus()
        })
    }
})

// reload button
document.querySelector('#reload-button').addEventListener('click', () => {
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

        const searchInput = document.getElementById(`${id}-search-input`)
        if (searchInput) {
            const inputContainer = searchInput.closest('.input-container')
            inputContainer.style.display = (id === tabId) ? 'block' : 'none'

            const clearButton = document.getElementById(`${id}-clear-button`)
            if (clearButton) {
                clearButton.style.display = (id === tabId && searchInput.value) ? 'inline-block' : 'none'
            }
        }
    })

    document.querySelectorAll('.tab-button').forEach(button => {
        button.classList.remove('active-tab-button')
    })

    document.getElementById(`${tabId}-button`).classList.add('active-tab-button')
    updateRowCount()
}

function updateRowCount() {
    const activeTab = document.querySelector('.active-tab-button').id
    let gridApi
    if (activeTab === 'records-button') {
        gridApi = recordsGrid
    } else if (activeTab === 'manifest-button') {
        gridApi = manifestGrid
    } else if (activeTab === 'log-button') {
        gridApi = logTextGrid
    }
    const count = gridApi ? gridApi.getDisplayedRowCount() : 0
    document.getElementById('row-count').textContent = count
}