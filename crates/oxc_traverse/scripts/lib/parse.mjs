import {readFile} from 'fs/promises';
import {join as pathJoin} from 'path';
import {fileURLToPath} from 'url';
import assert from 'assert';

export default async function getTypesFromCode() {
    const codeDirPath = pathJoin(fileURLToPath(import.meta.url), '../../../../oxc_ast/src/ast/');
    const filenames = ['js.rs', 'jsx.rs', 'literal.rs', 'ts.rs'];

    // Parse type defs from Rust files
    const types = Object.create(null);
    for (const filename of filenames) {
        const code = await readFile(`${codeDirPath}${filename}`, 'utf8'),
            lines = code.split(/\r?\n/);
        for (let i = 0; i < lines.length; i++) {
            if (lines[i] === '#[visited_node]') {
                let match;
                while (true) {
                    match = lines[++i].match(/^pub (enum|struct) (.+?)(<'a>)? \{/);
                    if (match) break;
                }
                const [, kind, name, lifetimeStr] = match,
                    hasLifetime = !!lifetimeStr;
                const itemLines = [];
                while (true) {
                    const line = lines[++i].replace(/\/\/.*$/, '').replace(/\s+/g, ' ').trim();
                    if (line === '}') break;
                    if (line !== '') itemLines.push(line);
                }

                if (kind === 'enum') {
                    const variants = [],
                        inherits = [];
                    for (const line of itemLines) {
                        const match = line.match(/^(.+?)\((.+?)\)(?: ?= ?(\d+))?,$/);
                        if (match) {
                            let [, name, type, discriminant] = match;
                            type = type.replace(/<'a>/g, '').replace(/<'a,\s*/g, '<');
                            discriminant = discriminant ? +discriminant : null;
                            variants.push({name, type, discriminant});
                        } else {
                            const match2 = line.match(/^@inherit ([A-Za-z]+)$/);
                            assert(match2, `Cannot parse line ${i} in '${filename}' as enum variant: '${line}'`);
                            inherits.push(match2[1]);
                        }
                    }
                    types[name] = {kind: 'enum', name, hasLifetime, variants, inherits};
                } else {
                    const fields = [];
                    for (let i = 0; i < itemLines.length; i++) {
                        const line = itemLines[i];
                        if (line.startsWith('#[')) {
                            while (!itemLines[i].endsWith(']')) {
                                i++;
                            }
                            continue;
                        }

                        const match = line.match(/^pub ((?:r#)?([a-z_]+)): (.+),(?: ?\/\/.+)?$/);
                        assert(match, `Cannot parse line ${i} in '${filename}' as struct field: '${line}'`);
                        const [, rawName, name, rawType] = match,
                            type = rawType.replace(/<'a>/g, '').replace(/<'a, ?/g, '<');
                        fields.push({name, type, rawName, rawType});
                    }
                    types[name] = {kind: 'struct', name, hasLifetime, fields};
                }
            }
        }
    }
    return types;
}
