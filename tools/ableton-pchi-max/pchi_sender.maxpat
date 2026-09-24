{
	"patcher" : 	{
		"fileversion" : 1,
		"appversion" : 		{
			"major" : 8,
			"minor" : 5,
			"revision" : 0,
			"architecture" : "x64",
			"modernui" : 1
		},
		"classnamespace" : "box",
		"rect" : [ 100.0, 100.0, 640.0, 480.0 ],
		"bglocked" : 0,
		"openinpresentation" : 0,
		"default_fontsize" : 12.0,
		"default_fontface" : 0,
		"default_fontname" : "Arial",
		"gridonopen" : 1,
		"gridsize" : [ 15.0, 15.0 ],
		"gridsnaponopen" : 1,
		"objectsnaponopen" : 1,
		"statusbarvisible" : 2,
		"toolbarvisible" : 1,
		"lefttoolbarpinned" : 0,
		"toptoolbarpinned" : 0,
		"righttoolbarpinned" : 0,
		"bottomtoolbarpinned" : 0,
		"toolbars_unpinned_last_save" : 0,
		"tallnewobj" : 0,
		"boxanimatetime" : 200,
		"enablehscroll" : 1,
		"enablevscroll" : 1,
		"devicewidth" : 0.0,
		"description" : "",
		"digest" : "",
		"tags" : "",
		"style" : "",
		"subpatcher_template" : "",
		"assistshowspatchername" : 0,
		"boxes" : [ 			{
				"box" : 				{
					"id" : "obj-1",
					"maxclass" : "newobj",
					"text" : "pchi_sender",
					"numinlets" : 1,
					"numoutlets" : 1,
					"outlettype" : [ "" ],
					"patching_rect" : [ 150.0, 300.0, 70.0, 22.0 ],
					"saved_object_attributes" : 					{
						"parameter_enable" : 0,
						"parameter_mappable" : 0
					}
,
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-2",
					"maxclass" : "comment",
					"text" : "PCHI Sender for Ableton Live",
					"numinlets" : 1,
					"numoutlets" : 0,
					"patching_rect" : [ 150.0, 50.0, 200.0, 20.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-3",
					"maxclass" : "comment",
					"text" : "Sends PCHI messages to PCHI Conductor",
					"numinlets" : 1,
					"numoutlets" : 0,
					"patching_rect" : [ 150.0, 75.0, 200.0, 20.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-4",
					"maxclass" : "newobj",
					"text" : "udpsend 127.0.0.1 8888",
					"numinlets" : 1,
					"numoutlets" : 0,
					"patching_rect" : [ 150.0, 350.0, 150.0, 22.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-5",
					"maxclass" : "newobj",
					"text" : "jit.pwindow 150 100",
					"numinlets" : 1,
					"numoutlets" : 2,
					"outlettype" : [ "jit_matrix", "" ],
					"patching_rect" : [ 400.0, 200.0, 150.0, 100.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-6",
					"maxclass" : "newobj",
					"text" : "live.object",
					"numinlets" : 2,
					"numoutlets" : 2,
					"outlettype" : [ "", "" ],
					"patching_rect" : [ 150.0, 150.0, 70.0, 22.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-7",
					"maxclass" : "newobj",
					"text" : "live.path",
					"numinlets" : 1,
					"numoutlets" : 2,
					"outlettype" : [ "", "" ],
					"patching_rect" : [ 150.0, 120.0, 70.0, 22.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-8",
					"maxclass" : "newobj",
					"text" : "live.observer",
					"numinlets" : 1,
					"numoutlets" : 2,
					"outlettype" : [ "", "" ],
					"patching_rect" : [ 150.0, 180.0, 80.0, 22.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-9",
					"maxclass" : "comment",
					"text" : "Configure PCHI Conductor settings",
					"numinlets" : 1,
					"numoutlets" : 0,
					"patching_rect" : [ 400.0, 50.0, 200.0, 20.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-10",
					"maxclass" : "comment",
					"text" : "Host: 127.0.0.1",
					"numinlets" : 1,
					"numoutlets" : 0,
					"patching_rect" : [ 400.0, 75.0, 100.0, 20.0 ],
					"style" : ""
				}

			}
, 			{
				"box" : 				{
					"id" : "obj-11",
					"maxclass" : "comment",
					"text" : "Port: 8888",
					"numinlets" : 1,
					"numoutlets" : 0,
					"patching_rect" : [ 400.0, 95.0, 100.0, 20.0 ],
					"style" : ""
				}

			}
 ],
		"lines" : [ 			{
				"patchline" : 				{
					"destination" : [ "obj-4", 0 ],
					"source" : [ "obj-1", 0 ]
				}

			}
, 			{
				"patchline" : 				{
					"destination" : [ "obj-1", 0 ],
					"source" : [ "obj-8", 0 ]
				}

			}
 ],
		"dependency_cache" : [ 			{
				"name" : "jit.pwindow.mxo",
				"type" : "iLaX",
				"version" : 1
			}
, 			{
				"name" : "live.object.mxo",
				"type" : "iLaX",
				"version" : 1
			}
, 			{
				"name" : "live.path.mxo",
				"type" : "iLaX",
				"version" : 1
			}
, 			{
				"name" : "live.observer.mxo",
				"type" : "iLaX",
				"version" : 1
			}
, 			{
				"name" : "udpsend.mxo",
				"type" : "iLaX",
				"version" : 1
			}
 ],
		"autosave" : 0
	}
}
