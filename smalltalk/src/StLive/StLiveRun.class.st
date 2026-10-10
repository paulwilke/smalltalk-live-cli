"
Support for `stlive run`: a one-shot headless run of a program loaded from Tonel sources.
The CLI passes everything through environment variables; results, text for stdout/stderr and the exit code are
written to files (UTF-8) so Pharo's own console chatter never mixes with the program's output.
Class side only.
"
Class {
	#name : 'StLiveRun',
	#superclass : 'Object',
	#classInstVars : [
		'directory',
		'out',
		'err',
		'exitCode',
		'arguments'
	],
	#package : 'StLive'
}

{ #category : 'running' }
StLiveRun class >> beginIn: aDirectoryPath [
	| env |
	directory := aDirectoryPath asFileReference.
	out := WriteStream on: String new.
	err := WriteStream on: String new.
	exitCode := 0.
	env := Smalltalk os environment.
	arguments := (STON fromString: (env at: 'STLIVE_RUN_ARGS' ifAbsent: [ '[]' ])) asArray collect: [ :a | a asString ]
]

{ #category : 'accessing' }
StLiveRun class >> arguments [
	"The arguments given after -- on the command line, as an Array of Strings."
	^ arguments ifNil: [ #() ]
]

{ #category : 'accessing' }
StLiveRun class >> stdout: aString [
	"Text for the program's standard output (written as UTF-8)."
	out nextPutAll: aString asString
]

{ #category : 'accessing' }
StLiveRun class >> stderr: aString [
	err nextPutAll: aString asString
]

{ #category : 'accessing' }
StLiveRun class >> exit: anInteger [
	"End the run with this exit code (output so far is kept)."
	exitCode := anInteger.
	self finish.
	Smalltalk exit: anInteger
]

{ #category : 'running' }
StLiveRun class >> loadTonelFrom: aDirectoryPath packages: packageNames [
	"Load packages from a Tonel directory (one folder per package). Without names every package folder is loaded."
	| repo names |
	repo := TonelRepository new directory: aDirectoryPath asFileReference; yourself.
	names := packageNames isEmpty ifTrue: [ repo allFileNames asArray ] ifFalse: [ packageNames ].
	names do: [ :name | (repo versionFrom: name) load ]
]

{ #category : 'running' }
StLiveRun class >> result: anObject [
	"The value of the evaluated expression becomes standard output: Strings as they are, nil nothing, anything else printString."
	anObject isNil ifTrue: [ ^ self ].
	out nextPutAll: (anObject isString ifTrue: [ anObject asString ] ifFalse: [ anObject printString ])
]

{ #category : 'running' }
StLiveRun class >> failed: anException [
	exitCode := 1.
	err nextPutAll: anException class name , ': ' , ([ anException messageText asString ] on: Error do: [ :e | '?' ]); lf.
	"The frames of the program itself: stop where the doit machinery of this run script begins."
	[ | ctx n |
	ctx := anException signalerContext.
	n := 0.
	[ ctx notNil and: [ n < 25 and: [ (ctx printString includesSubstring: 'OCReceiverDoItSemanticScope') not ] ] ] whileTrue: [
		| line |
		line := ctx printString lines first.
		err nextPutAll: '  ' , (line size > 140 ifTrue: [ (line first: 140) , '...' ] ifFalse: [ line ]); lf.
		n := n + 1.
		ctx := ctx sender ] ]
		on: Error
		do: [ :e | nil ]
]

{ #category : 'running' }
StLiveRun class >> finish [
	"Write the collected output; safe to call more than once."
	directory ifNil: [ ^ self ].
	self write: out contents to: 'out.txt'.
	self write: err contents to: 'err.txt'.
	self write: exitCode printString to: 'exit.txt'
]

{ #category : 'private' }
StLiveRun class >> write: aString to: aName [
	| file |
	file := directory / aName.
	file ensureDelete.
	file binaryWriteStreamDo: [ :s | s nextPutAll: aString utf8Encoded ]
]
