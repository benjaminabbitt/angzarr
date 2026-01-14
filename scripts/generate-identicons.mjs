#!/usr/bin/env node
import { writeFileSync, mkdirSync } from 'fs';
import * as jdenticon from 'jdenticon';

const services = [
    'angzarr-gateway',
    'angzarr-command',
    'angzarr-projector',
    'angzarr-saga'
];

const outputDir = process.argv[2] || './deploy/helm/angzarr/icons';

try {
    mkdirSync(outputDir, { recursive: true });
} catch {}

for (const service of services) {
    const svg = jdenticon.toSvg(service, 200);
    const path = `${outputDir}/${service}.svg`;
    writeFileSync(path, svg);
    console.log(`Generated: ${path}`);
}
