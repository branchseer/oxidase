import { markdownTable } from 'markdown-table'
import fs from 'node:fs'

export interface Data {
    [group: string]: {
        [bench: string]: {
            [parameter: string]: number
        }
    }
}


export async function readdir(dir: string): Promise<string[]> {
    return (await fs.promises.readdir(dir)).filter(
      name => !name.startsWith('.') // filtering out .DS_Store
    ).sort();
}

export function formatAsTable(data: Data, formatNumber: (n: number) => string) {
    const groups = Object.keys(data);
    let out = "";

    for (const group of groups) {
        const columns = Object.keys(data[group]);
        const rows = Object.keys(data[group][columns[0]]);

        out += `### ${group}\n`;
        const table = [["", ...columns]];

        for (const row of rows) {
            const columnValues = columns.map((column) => data[group][column][row]);
            const baseline = columnValues[0];
            const column_values = columnValues.map((number) => {
                if (number === undefined) {
                    return 'N/A'
                }
                return `\`${formatNumber(number)}\` (${(number / baseline).toFixed(2)}x)`
            });
            table.push([row, ...column_values]);
        }
        out += markdownTable(table) + '\n';
    }
    return out;
}
