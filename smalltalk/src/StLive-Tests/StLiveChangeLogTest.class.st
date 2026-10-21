Class {
	#name : 'StLiveChangeLogTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'helpers' }
StLiveChangeLogTest >> recordChangeIn: aPackage saved: aBoolean [
	server changes add: (commands m: { 'package' -> aPackage. 'saved' -> aBoolean })
]

{ #category : 'changes by package' }
StLiveChangeLogTest >> testChangesByPackageCountsTotalAndUnsaved [
	| counts |
	self recordChangeIn: 'P' saved: false.
	self recordChangeIn: 'P' saved: true.
	self recordChangeIn: 'Q' saved: false.
	counts := commands changesByPackage.
	self assert: ((counts at: 'P') at: 'total') equals: 2.
	self assert: ((counts at: 'P') at: 'unsaved') equals: 1.
	self assert: ((counts at: 'Q') at: 'unsaved') equals: 1
]

{ #category : 'changes by package' }
StLiveChangeLogTest >> testChangeWithoutPackageIsCountedUnderQuestionMark [
	server changes add: (commands m: { 'saved' -> false }).
	self assert: ((commands changesByPackage at: '?') at: 'unsaved') equals: 1
]

{ #category : 'supersede' }
StLiveChangeLogTest >> testSupersedeMarksOnlyUnsavedChangesOfTheGivenPackages [
	| n |
	self recordChangeIn: 'P' saved: false.
	self recordChangeIn: 'P' saved: true.
	self recordChangeIn: 'Q' saved: false.
	n := commands supersedeChangesOf: #('P').
	self assert: n equals: 1.
	self assert: (server changes first at: 'saved').
	self assert: (server changes first at: 'superseded_by') equals: 'load'.
	self assert: (server changes third at: 'saved') not
]

{ #category : 'supersede' }
StLiveChangeLogTest >> testSupersedeAnswersZeroWhenNothingMatches [
	self recordChangeIn: 'P' saved: false.
	self assert: (commands supersedeChangesOf: #('Q')) equals: 0.
	self assert: (server changes first at: 'saved') not
]

{ #category : 'supersede' }
StLiveChangeLogTest >> testSupersededChangesNoLongerCountAsUnsaved [
	self recordChangeIn: 'P' saved: false.
	commands supersedeChangesOf: #('P').
	self assert: ((commands changesByPackage at: 'P') at: 'unsaved') equals: 0
]
