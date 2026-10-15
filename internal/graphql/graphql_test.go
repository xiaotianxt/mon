package graphql

import (
	"strings"
	"testing"

	"github.com/xiaotianxt/mon/internal/jsonx"
)

func mustParse(t *testing.T, status int, body string) *ResponseError {
	t.Helper()
	response, err := jsonx.Decode([]byte(body))
	if err != nil {
		t.Fatal(err)
	}
	responseErr, err := ParseResponseError("Example", status, response)
	if err != nil {
		t.Fatal(err)
	}
	return responseErr
}

func TestClassifiesSchemaValidationErrorsByExtensionCode(t *testing.T) {
	err := mustParse(t, 200, `{"errors":[{"message":"Cannot query field isDisabled.","extensions":{"code":"GRAPHQL_VALIDATION_FAILED"}}]}`)
	if err.Class() != SchemaIncompatible {
		t.Fatalf("class = %v", err.Class())
	}
	if !strings.Contains(err.Error(), "schema is incompatible") {
		t.Fatalf("message = %q", err.Error())
	}
}

func TestClassifiesHTTP400GraphQLErrorsAsSchemaIncompatible(t *testing.T) {
	err := mustParse(t, 400, `{"errors":[{"message":"Something went wrong while processing.","locations":[{"line":1,"column":34}]}]}`)
	if err.Class() != SchemaIncompatible {
		t.Fatalf("class = %v", err.Class())
	}
}

func TestClassifiesBadUserInputAsDefinitiveRejection(t *testing.T) {
	err := mustParse(t, 200, `{"errors":[{"message":"Invalid category","extensions":{"code":"BAD_USER_INPUT"}}]}`)
	if err.Class() != DefinitiveRejection {
		t.Fatalf("class = %v", err.Class())
	}
}

func TestUnknownTopLevelErrorCodesRemainAmbiguous(t *testing.T) {
	err := mustParse(t, 200, `{"errors":[{"message":"Resolver stopped unexpectedly","extensions":{"code":"INTERNAL_ERROR"}}]}`)
	if err.Class() != Ambiguous {
		t.Fatalf("class = %v", err.Class())
	}
}

func TestAcceptsAnEmptyErrorsArray(t *testing.T) {
	if err := mustParse(t, 200, `{"data":{},"errors":[]}`); err != nil {
		t.Fatalf("unexpected error %v", err)
	}
}

func TestRejectsNullAndMalformedErrors(t *testing.T) {
	for _, body := range []string{`{"errors":null}`, `{"errors":[{"extensions":{}}]}`, `{"errors":"boom"}`} {
		response, _ := jsonx.Decode([]byte(body))
		if _, err := ParseResponseError("Example", 200, response); err == nil {
			t.Fatalf("expected parse failure for %s", body)
		}
	}
}
