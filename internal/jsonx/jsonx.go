// Package jsonx provides dynamic JSON helpers that preserve number precision
// and never HTML-escape output, so Monarch payloads round-trip verbatim.
package jsonx

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strconv"
)

// Decode parses exactly one JSON value. Numbers decode as json.Number so
// large integers and decimal formatting survive re-encoding.
func Decode(data []byte) (any, error) {
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.UseNumber()
	var v any
	if err := dec.Decode(&v); err != nil {
		return nil, err
	}
	if _, err := dec.Token(); !errors.Is(err, io.EOF) {
		return nil, errors.New("trailing data after JSON value")
	}
	return v, nil
}

// Convert re-decodes a dynamic value into a typed destination.
func Convert(v any, dst any) error {
	raw, err := json.Marshal(v)
	if err != nil {
		return err
	}
	return json.Unmarshal(raw, dst)
}

// Compact encodes v on one line.
func Compact(v any) string {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(v); err != nil {
		return fmt.Sprintf("<unencodable: %v>", err)
	}
	return string(bytes.TrimSuffix(buf.Bytes(), []byte("\n")))
}

// WritePretty writes v with two-space indentation and a trailing newline.
func WritePretty(w io.Writer, v any) error {
	enc := json.NewEncoder(w)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", "  ")
	return enc.Encode(v)
}

// Get walks object keys, returning nil when any step is absent or not an object.
func Get(v any, path ...string) any {
	for _, key := range path {
		obj, ok := v.(map[string]any)
		if !ok {
			return nil
		}
		v = obj[key]
	}
	return v
}

// Lookup returns the member and whether the key is present, even if its value is null.
func Lookup(v any, key string) (any, bool) {
	obj, ok := v.(map[string]any)
	if !ok {
		return nil, false
	}
	member, ok := obj[key]
	return member, ok
}

func String(v any) (string, bool) {
	s, ok := v.(string)
	return s, ok
}

func Bool(v any) (bool, bool) {
	b, ok := v.(bool)
	return b, ok
}

func Float(v any) (float64, bool) {
	switch n := v.(type) {
	case json.Number:
		f, err := n.Float64()
		return f, err == nil
	case float64:
		return n, true
	}
	return 0, false
}

func Uint(v any) (uint64, bool) {
	switch n := v.(type) {
	case json.Number:
		u, err := strconv.ParseUint(n.String(), 10, 64)
		return u, err == nil
	case float64:
		if n < 0 || n != float64(uint64(n)) {
			return 0, false
		}
		return uint64(n), true
	}
	return 0, false
}
