package main

import "errors"

var (
	errSignatureInvalid   = errors.New("signature is invalid")
	errLoadedSuccessfully = errors.New("loaded successfully")
)
