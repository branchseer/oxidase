import { expose } from "threads/worker";
import { transpile } from './tsc.ts'

const worker = {
    transpileFile(path: string) {
        let source: string
        try {
            source = Deno.readTextFileSync(path)
        }
        catch {
            return null
        }
        return transpile(source)
    }
}

expose(worker);

export type Worker = typeof worker;
