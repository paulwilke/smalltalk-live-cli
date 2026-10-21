Class {
	#name : 'StLiveDescriberTest',
	#superclass : 'StLiveTestCase',
	#instVars : [
		'describer'
	],
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveDescriberTest >> setUp [
	super setUp.
	describer := StLiveDescriber on: server
]

{ #category : 'inline' }
StLiveDescriberTest >> testImmediateValuesAndShortStringsAreInline [
	self assert: (describer isInline: nil).
	self assert: (describer isInline: true).
	self assert: (describer isInline: 3).
	self assert: (describer isInline: $a).
	self assert: (describer isInline: 'short')
]

{ #category : 'inline' }
StLiveDescriberTest >> testLongStringsAndObjectsAreNotInline [
	self deny: (describer isInline: (String new: 101 withAll: $a)).
	self deny: (describer isInline: Object new)
]

{ #category : 'print' }
StLiveDescriberTest >> testPrintIsLimited [
	self assert: (describer print: (1 to: 100) asArray limit: 20) size <= 20
]

{ #category : 'print' }
StLiveDescriberTest >> testFailingPrintIsReportedAsText [
	| cls |
	cls := self fixtureClassNamed: 'StLiveTestsBadPrint' superclass: Object package: 'StLiveTestsFixture'.
	self compile: 'printOn: aStream
	Error signal: ''no print''' in: cls.
	self assert: (describer print: cls new limit: 50) equals: '<printString failed: Error>'
]

{ #category : 'summary' }
StLiveDescriberTest >> testSummaryOfAnInlineValueHasNoReference [
	| summary |
	summary := describer summary: 3.
	self assert: (summary at: 'class') equals: 'SmallInteger'.
	self assert: (summary at: 'print') equals: '3'.
	self deny: (summary includesKey: 'ref')
]

{ #category : 'summary' }
StLiveDescriberTest >> testSummaryOfAnObjectHasAReference [
	self assert: (((describer summary: Object new) at: 'ref') beginsWith: 'obj-')
]

{ #category : 'summary' }
StLiveDescriberTest >> testSummaryOfAShortValueIsNotMarkedTruncated [
	self deny: ((describer summary: 3) includesKey: 'print_truncated')
]

{ #category : 'summary' }
StLiveDescriberTest >> testSummaryOfALongStringIsMarkedTruncatedWithAHint [
	| summary |
	summary := describer summary: (String new: 200 withAll: $a).
	self assert: (summary at: 'print_truncated').
	self assert: ((summary at: 'hint') includesSubstring: 'obj text')
]

{ #category : 'items' }
StLiveDescriberTest >> testItemsArePagedWithANextOffset [
	| page |
	page := describer items: #(10 20 30 40 50) offset: 2 limit: 2.
	self assert: (page at: 'total') equals: 5.
	self assert: (page at: 'items') size equals: 2.
	self assert: ((page at: 'items') first at: 'index') equals: 3.
	self assert: (((page at: 'items') first at: 'value') at: 'print') equals: '30'.
	self assert: (page at: 'next_offset') equals: 4.
	self assert: (page at: 'truncated')
]

{ #category : 'items' }
StLiveDescriberTest >> testLastPageHasNoNextOffset [
	| page |
	page := describer items: #(10 20 30 40 50) offset: 4 limit: 10.
	self assert: (page at: 'items') size equals: 1.
	self deny: (page at: 'truncated').
	self deny: (page includesKey: 'next_offset')
]

{ #category : 'items' }
StLiveDescriberTest >> testItemsOfADictionaryAreKeyAndValue [
	| item |
	item := (describer items: (Dictionary new at: #a put: 1; yourself) offset: 0 limit: 5) at: 'items'.
	self assert: item first keys asArray equals: #('key' 'value')
]

{ #category : 'items' }
StLiveDescriberTest >> testItemsOfANonCollectionFail [
	| failure |
	failure := self failureOf: [ describer items: Object new offset: 0 limit: 5 ].
	self assert: failure code equals: 'not_a_collection'
]
