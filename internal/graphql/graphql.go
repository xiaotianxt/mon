// Package graphql classifies top-level GraphQL error envelopes.
package graphql

import (
	"fmt"
	"strings"

	"github.com/xiaotianxt/mon/internal/jsonx"
)

type ErrorClass int

const (
	Ambiguous ErrorClass = iota
	SchemaIncompatible
	DefinitiveRejection
)

type errorItem struct {
	Message    *string `json:"message"`
	Extensions *struct {
		Code *string `json:"code"`
	} `json:"extensions"`
}

func (e errorItem) code() string {
	if e.Extensions == nil || e.Extensions.Code == nil {
		return ""
	}
	return *e.Extensions.Code
}

// ResponseError is a non-empty top-level "errors" array from a GraphQL response.
type ResponseError struct {
	Operation string
	// HTTPStatus is 0 when the status is unknown.
	HTTPStatus int
	errors     []errorItem
}

func (e *ResponseError) Class() ErrorClass {
	var codes []string
	for _, item := range e.errors {
		if code := item.code(); code != "" {
			codes = append(codes, code)
		}
	}

	if len(codes) > 0 && all(codes, func(c string) bool {
		return c == "GRAPHQL_VALIDATION_FAILED" || c == "GRAPHQL_PARSE_FAILED"
	}) {
		return SchemaIncompatible
	}
	if len(codes) > 0 && all(codes, func(c string) bool { return c == "BAD_USER_INPUT" }) {
		return DefinitiveRejection
	}

	// Monarch omits extension codes for document validation failures and
	// answers HTTP 400 with a generic GraphQL error; treat that envelope as an
	// incompatible operation.
	if e.HTTPStatus == 400 {
		return SchemaIncompatible
	}
	return Ambiguous
}

func (e *ResponseError) Error() string {
	descriptions := make([]string, 0, len(e.errors))
	for _, item := range e.errors {
		if code := item.code(); code != "" {
			descriptions = append(descriptions, fmt.Sprintf("[%s] %s", code, *item.Message))
		} else {
			descriptions = append(descriptions, *item.Message)
		}
	}
	joined := strings.Join(descriptions, "; ")

	if e.Class() == SchemaIncompatible {
		return fmt.Sprintf("Monarch schema is incompatible with GraphQL operation %s: %s", e.Operation, joined)
	}
	return fmt.Sprintf("GraphQL %s returned errors: %s", e.Operation, joined)
}

// ParseResponseError returns nil when the response has no errors member or an
// empty one, and fails closed when the member is present but malformed.
func ParseResponseError(operation string, httpStatus int, response any) (*ResponseError, error) {
	raw, ok := jsonx.Lookup(response, "errors")
	if !ok {
		return nil, nil
	}
	if raw == nil {
		return nil, fmt.Errorf("GraphQL %s response contained null errors", operation)
	}

	var items []errorItem
	if err := jsonx.Convert(raw, &items); err != nil {
		return nil, fmt.Errorf("failed to parse GraphQL errors for %s: %w", operation, err)
	}
	for i, item := range items {
		if item.Message == nil {
			return nil, fmt.Errorf("failed to parse GraphQL errors for %s: error %d has no message", operation, i)
		}
	}
	if len(items) == 0 {
		return nil, nil
	}

	return &ResponseError{Operation: operation, HTTPStatus: httpStatus, errors: items}, nil
}

func all(values []string, pred func(string) bool) bool {
	for _, v := range values {
		if !pred(v) {
			return false
		}
	}
	return true
}
