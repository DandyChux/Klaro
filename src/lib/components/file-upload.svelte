<script lang="ts">
	import * as Card from "$lib/components/ui/card";
	import { Button } from "$lib/components/ui/button";
	import { Badge } from "$lib/components/ui/badge";

	type Props = {
		supportedFormats: SupportedFormat[];
		onFilesSelected: (files: UploadedFile[]) => void;
	};

	let { supportedFormats = [], onFilesSelected }: Props = $props();

	interface SupportedFormat {
		extension: string;
		name: string;
		mime_type: string;
	}

	interface UploadedFile {
		name: string;
		size: number;
		type: string;
		data: string;
	}

	let isDragging = $state(false);
	let fileInput: HTMLInputElement;

	function handleDragOver(e: DragEvent) {
		e.preventDefault();
		isDragging = true;
	}

	function handleDragLeave(e: DragEvent) {
		e.preventDefault();
		isDragging = false;
	}

	async function handleDrop(e: DragEvent) {
		e.preventDefault();
		isDragging = false;

		const files = e.dataTransfer?.files;
		if (files) {
			await processFiles(files);
		}
	}

	async function handleFileSelect(e: Event) {
		const target = e.target as HTMLInputElement;
		if (target.files) {
			await processFiles(target.files);
		}
	}

	async function processFiles(fileList: FileList) {
		const processed: UploadedFile[] = [];

		for (let i = 0; i < fileList.length; i++) {
			const file = fileList[i];
			const data = await readFileAsBase64(file);
			processed.push({
				name: file.name,
				size: file.size,
				type: file.type,
				data,
			});
		}

		onFilesSelected(processed);
	}

	function readFileAsBase64(file: File): Promise<string> {
		return new Promise((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => {
				const result = reader.result as string;
				// Remove the data URL prefix (e.g., "data:text/csv;base64,")
				const base64 = result.split(",")[1];
				resolve(base64);
			};
			reader.onerror = reject;
			reader.readAsDataURL(file);
		});
	}

	function openFileDialog() {
		fileInput.click();
	}

	let acceptedExtensions = $derived(
		supportedFormats.map((f) => `.${f.extension}`).join(","),
	);
</script>

<Card.Root>
	<Card.Header>
		<Card.Title>Upload Files</Card.Title>
		<Card.Description>
			Drag and drop your files or click to browse. We'll scan them for
			personal information.
		</Card.Description>
	</Card.Header>
	<Card.Content>
		<div
			class="border-2 border-dashed rounded-lg p-12 text-center transition-all cursor-pointer hover:border-primary/50 hover:bg-muted/50 {isDragging
				? 'border-primary bg-primary/5 drop-zone-active'
				: 'border-muted-foreground/25'}"
			ondragover={handleDragOver}
			ondragleave={handleDragLeave}
			ondrop={handleDrop}
			onclick={openFileDialog}
			role="button"
			tabindex="0"
			onkeypress={(e) => e.key === "Enter" && openFileDialog()}
		>
			<input
				type="file"
				bind:this={fileInput}
				onchange={handleFileSelect}
				accept={acceptedExtensions}
				multiple
				class="hidden"
			/>

			<div class="flex flex-col items-center space-y-4">
				<div
					class="w-16 h-16 bg-primary/10 rounded-full flex items-center justify-center"
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-8 w-8 text-primary"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						stroke-width="2"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
						/>
					</svg>
				</div>

				<div>
					<p class="text-lg font-medium">
						{#if isDragging}
							Drop files here...
						{:else}
							Drag & drop files here
						{/if}
					</p>
					<p class="text-sm text-muted-foreground mt-1">
						or click to browse
					</p>
				</div>

				<Button variant="secondary" type="button">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-4 w-4 mr-2"
						viewBox="0 0 20 20"
						fill="currentColor"
					>
						<path
							fill-rule="evenodd"
							d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM6.293 6.707a1 1 0 010-1.414l3-3a1 1 0 011.414 0l3 3a1 1 0 01-1.414 1.414L11 5.414V13a1 1 0 11-2 0V5.414L7.707 6.707a1 1 0 01-1.414 0z"
							clip-rule="evenodd"
						/>
					</svg>
					Select Files
				</Button>
			</div>
		</div>

		<div class="mt-6">
			<p class="text-sm font-medium mb-2">Supported formats:</p>
			<div class="flex flex-wrap gap-2">
				{#each supportedFormats as format}
					<Badge variant="secondary">.{format.extension}</Badge>
				{/each}
			</div>
		</div>
	</Card.Content>
</Card.Root>
