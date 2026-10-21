Class {
	#name : 'BaselineOfStLive',
	#superclass : 'BaselineOf',
	#package : 'BaselineOfStLive'
}

{ #category : 'baselines' }
BaselineOfStLive >> baseline: spec [
	<baseline>
	spec for: #common do: [
		spec package: 'StLive'.
		spec package: 'StLive-Tests' with: [ spec requires: #('StLive') ].
		"stlive init and attach load the default group: the framework only. Tests load with --group Tests."
		spec group: 'default' with: #('StLive').
		spec group: 'Tests' with: #('default' 'StLive-Tests') ]
]
