#!/usr/bin/env node
/**
 * create-schedule-example.js
 *
 * Example script: create a vesting schedule via the backend REST API.
 *
 * Usage:
 *   node scripts/create-schedule-example.js
 *
 * Prerequisites:
 *   1. Start the backend first:
 *        cd backend && npm install && npm start
 *   2. Optionally set API_URL if the backend runs on a different host/port:
 *        API_URL=http://localhost:3001 node scripts/create-schedule-example.js
 *
 * The script uses the built-in Node.js fetch API (Node 18+).
 * No external npm dependencies are required.
 *
 * Replace the placeholder values below with real Stellar addresses and a
 * deployed contract ID before pointing this at a live network.
 */

'use strict';

// ── Configuration ─────────────────────────────────────────────────────────────

const API_URL = process.env.API_URL || 'http://localhost:3001';

/**
 * Placeholder Stellar addresses matching the pattern in .env.example.
 * Replace these with real Testnet/Mainnet addresses for a live run.
 *
 * Stellar contract/account IDs are 56-character uppercase base-32 strings
 * starting with 'C' (contracts) or 'G' (accounts).
 */
const EXAMPLE_CONTRACT_ID  = 'CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF'; // vesting contract
const EXAMPLE_FROM         = 'GBXXXXXADMINADDRESSXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX'; // token funder/admin
const EXAMPLE_BENEFICIARY  = 'GBXXXXXBENEFICIARYADDRESSXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX'; // vesting recipient
const EXAMPLE_TOKEN_ADDR   = 'CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH'; // token contract

// Vesting parameters
const TOTAL_AMOUNT    = 1_000_000;           // token amount (in smallest units / stroops)
const START_TIME      = Math.floor(Date.now() / 1000); // Unix timestamp — now
const CLIFF_DURATION  = 31_536_000;           // 1 year in seconds
const TOTAL_DURATION  = 126_144_000;          // 4 years in seconds

// ── Request ───────────────────────────────────────────────────────────────────

async function main() {
  const endpoint = `${API_URL}/api/vesting/schedule`;

  const body = {
    contractId:    EXAMPLE_CONTRACT_ID,
    from:          EXAMPLE_FROM,
    beneficiary:   EXAMPLE_BENEFICIARY,
    tokenAddress:  EXAMPLE_TOKEN_ADDR,
    totalAmount:   TOTAL_AMOUNT,
    startTime:     START_TIME,
    cliffDuration: CLIFF_DURATION,
    totalDuration: TOTAL_DURATION,
  };

  console.log(`POST ${endpoint}`);
  console.log('Request body:', JSON.stringify(body, null, 2));

  let response;
  try {
    response = await fetch(endpoint, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify(body),
    });
  } catch (networkErr) {
    console.error(`\nNetwork error: ${networkErr.message}`);
    console.error('Is the backend running? Run: cd backend && npm start');
    process.exit(1);
  }

  let data;
  try {
    data = await response.json();
  } catch {
    console.error(`\nFailed to parse response as JSON (HTTP ${response.status})`);
    process.exit(1);
  }

  console.log(`\nHTTP ${response.status}`);
  console.log('Response:', JSON.stringify(data, null, 2));

  if (!response.ok) {
    console.error(`\nError: API returned HTTP ${response.status}`);
    process.exit(1);
  }

  console.log('\nSchedule creation queued successfully.');
}

main();
