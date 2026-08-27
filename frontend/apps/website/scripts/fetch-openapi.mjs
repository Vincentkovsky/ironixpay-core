#!/usr/bin/env node
/**
 * Fetch the latest OpenAPI spec from sandbox and compare with local copy.
 *
 * Git strategy:
 *   - public/openapi.json is committed to git as a FALLBACK so that builds
 *     work without network access (offline, CI cache, etc.).
 *   - This script runs via `npm run prebuild` before every build.
 *     It fetches the latest spec from sandbox and overwrites the local copy
 *     only if the content has actually changed. This avoids noisy git diffs.
 *   - After a successful fetch, run `git diff public/openapi.json` to see
 *     what changed, then commit the update alongside your backend changes.
 */

import { readFileSync, writeFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const SPEC_PATH = resolve(__dirname, '../public/openapi.json')
const SPEC_URL = 'https://sandbox.ironixpay.com/docs/openapi.json'
const TIMEOUT_MS = 10_000

async function main() {
    console.log(`📡 Fetching OpenAPI spec from ${SPEC_URL}...`)

    try {
        const controller = new AbortController()
        const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)

        const res = await fetch(SPEC_URL, { signal: controller.signal })
        clearTimeout(timer)

        if (!res.ok) {
            throw new Error(`HTTP ${res.status} ${res.statusText}`)
        }

        const remote = await res.text()

        // Validate it's actually valid JSON with an openapi field
        const parsed = JSON.parse(remote)
        if (!parsed.openapi || !parsed.paths) {
            throw new Error('Response is not a valid OpenAPI spec')
        }

        // Compare with local copy to avoid unnecessary git diffs
        let local = ''
        try {
            local = readFileSync(SPEC_PATH, 'utf-8')
        } catch {
            // File doesn't exist yet — that's fine
        }

        // Normalize for comparison (minified JSON, no trailing newline)
        const remoteNormalized = JSON.stringify(parsed)
        let localNormalized = ''
        try {
            localNormalized = JSON.stringify(JSON.parse(local))
        } catch {
            // Local file is invalid JSON — overwrite it
        }

        if (remoteNormalized === localNormalized) {
            console.log('✅ OpenAPI spec is already up to date')
        } else {
            writeFileSync(SPEC_PATH, remoteNormalized)
            const operations = Object.values(parsed.paths)
                .flatMap((methods) => Object.values(methods))
                .filter((op) => op.operationId)
            console.log(`✅ OpenAPI spec updated (${operations.length} operations, ${(remoteNormalized.length / 1024).toFixed(1)}KB)`)
        }
    } catch (err) {
        console.warn(`⚠️  Fetch failed: ${err.message}`)
        console.warn('   Using existing local spec as fallback.')

        // Verify local fallback exists
        try {
            const local = readFileSync(SPEC_PATH, 'utf-8')
            JSON.parse(local)
            console.log('   ✅ Local fallback is valid.')
        } catch {
            console.error('   ❌ No valid local fallback! Build may fail.')
            process.exit(1)
        }
    }
}

main()
