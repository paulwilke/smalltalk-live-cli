Class {
	#name : 'StLiveCommandsHelpersTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'responses' }
StLiveCommandsHelpersTest >> testMKeepsTheOrderOfThePairs [
	| d |
	d := commands m: { 'b' -> 1. 'a' -> 2 }.
	self assert: d keys asArray equals: #('b' 'a')
]

{ #category : 'responses' }
StLiveCommandsHelpersTest >> testMAnswersTheValues [
	| d |
	d := commands m: { 'b' -> 1. 'a' -> 2 }.
	self assert: (d at: 'a') equals: 2
]

{ #category : 'responses' }
StLiveCommandsHelpersTest >> testOkWrapsTheResult [
	| response |
	response := commands ok: (commands m: { 'x' -> 1 }).
	self assert: response keys asArray equals: #('ok' 'result').
	self assert: (response at: 'ok') equals: true.
	self assert: ((response at: 'result') at: 'x') equals: 1
]

{ #category : 'failures' }
StLiveCommandsHelpersTest >> testFailCarriesCodeAndMessage [
	| failure |
	failure := self failureOf: [ commands fail: 'sample_code' message: 'Sample text' ].
	self assert: failure code equals: 'sample_code'.
	self assert: failure messageText equals: 'Sample text'
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testNeedAnswersThePresentValue [
	self assert: (commands need: 'a' in: (commands m: { 'a' -> 3 })) equals: 3
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testNeedFailsForAMissingKey [
	| failure |
	failure := self failureOf: [ commands need: 'a' in: (commands m: {}) ].
	self assert: failure code equals: 'missing_argument'.
	self assert: failure messageText equals: 'Missing argument: a'
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testNeedFailsForANilValue [
	| failure |
	failure := self failureOf: [ commands need: 'a' in: (commands m: { 'a' -> nil }) ].
	self assert: failure code equals: 'missing_argument'
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testIntUsesTheDefaultWhenMissingOrNil [
	self assert: (commands int: 'n' in: (commands m: {}) default: 7) equals: 7.
	self assert: (commands int: 'n' in: (commands m: { 'n' -> nil }) default: 7) equals: 7
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testIntKeepsIntegers [
	self assert: (commands int: 'n' in: (commands m: { 'n' -> 5 }) default: 0) equals: 5
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testIntParsesStrings [
	self assert: (commands int: 'n' in: (commands m: { 'n' -> '42' }) default: 0) equals: 42.
	self assert: (commands int: 'n' in: (commands m: { 'n' -> '-3' }) default: 0) equals: -3
]

{ #category : 'arguments' }
StLiveCommandsHelpersTest >> testIntRejectsText [
	| failure |
	failure := self failureOf: [ commands int: 'n' in: (commands m: { 'n' -> 'abc' }) default: 0 ].
	self assert: failure code equals: 'bad_argument'
]

{ #category : 'names' }
StLiveCommandsHelpersTest >> testNameMatchesSubstringsByDefault [
	self assert: (commands name: 'StLiveCartTest' matches: 'Cart').
	self deny: (commands name: 'StLiveCartTest' matches: 'Zebra')
]

{ #category : 'names' }
StLiveCommandsHelpersTest >> testNameMatchesGlobPatterns [
	self assert: (commands name: 'StLiveCartTest' matches: 'StLive*').
	self deny: (commands name: 'StLiveCartTest' matches: '*Zebra*').
	self assert: (commands name: 'Cart' matches: 'Car#')
]

{ #category : 'levenshtein' }
StLiveCommandsHelpersTest >> testLevenshteinOfKnownPairs [
	self assert: (commands levenshtein: 'kitten' to: 'sitting') equals: 3.
	self assert: (commands levenshtein: 'flaw' to: 'lawn') equals: 2
]

{ #category : 'levenshtein' }
StLiveCommandsHelpersTest >> testLevenshteinWithEmptyAndEqualStrings [
	self assert: (commands levenshtein: '' to: 'abc') equals: 3.
	self assert: (commands levenshtein: 'abc' to: 'abc') equals: 0
]
